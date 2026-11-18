//! Portable installation paths, discovery and validation.
//!
//! Phase 2 assumed one machine: V1's `config.json` at a fixed location supplied
//! every path. This module replaces that assumption with configuration the
//! operator owns, so a public build contains no personal directory at all.
//!
//! Two rules keep this safe:
//!
//!   * **No shell.** A path is only ever handed to `Command::new(exe).arg(..)`
//!     as a real argv element, never interpolated into a command line. The
//!     character checks in `check` are defence in depth on top of that, so that a
//!     value which could never be executed also cannot be silently accepted.
//!   * **Read, never write.** Selecting a path only records a string. Nothing
//!     here creates, moves, repairs or deletes anything inside an installation.

use std::path::{Component, Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use serde::{Deserialize, Serialize};

/// The configuration every part of the backend reads.
///
/// There is one singleton because the alternative is threading a path set through
/// four managers that are called from the supervisor thread, the command handlers
/// and the interface alike. `install` runs whenever settings are loaded or edited,
/// so exactly one place decides what the current configuration is.
static CURRENT: OnceLock<RwLock<Paths>> = OnceLock::new();

pub fn current() -> Paths {
    CURRENT
        .get()
        .and_then(|lock| lock.read().ok())
        .map(|guard| guard.clone())
        .unwrap_or_default()
}

pub fn install(paths: &Paths) {
    let lock = CURRENT.get_or_init(|| RwLock::new(paths.clone()));
    if let Ok(mut guard) = lock.write() {
        *guard = paths.clone();
    }
}

/// What a configured value is supposed to point at.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// An executable: must exist and be a file.
    Exe,
    /// Any other file: must exist.
    File,
    /// A directory: must exist.
    Dir,
    /// May be empty, otherwise validated like `Dir`.
    OptionalDir,
    /// May be empty, otherwise validated like `File`.
    OptionalFile,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Exe => "executable",
            Kind::File => "file",
            Kind::Dir => "folder",
            Kind::OptionalDir => "optional folder",
            Kind::OptionalFile => "optional file",
        }
    }

    pub fn optional(self) -> bool {
        matches!(self, Kind::OptionalDir | Kind::OptionalFile)
    }

    fn wants_dir(self) -> bool {
        matches!(self, Kind::Dir | Kind::OptionalDir)
    }
}

/// Every location the application needs, stored as text exactly as the operator
/// typed or picked it.
///
/// Empty means "not configured yet". Defaults are deliberately blank: a public
/// build must not guess at anybody's directory layout.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Paths {
    /// Root of the native-vLLM installation (the folder holding `build-info.json`).
    pub vllm_root: String,
    /// The `vllm.exe` launcher inside its Python environment.
    pub vllm_exe: String,
    /// The Python interpreter of that same environment.
    pub python_exe: String,
    /// The local model checkpoint directory.
    pub model_checkpoint: String,
    /// The YAML serve configuration passed to `vllm serve --config`.
    pub launch_config: String,
    /// The chat template passed to `--chat-template`.
    pub chat_template: String,
    /// The blackout engine script.
    pub blackout_engine: String,
    /// The interpreter used to run the blackout engine.
    pub blackout_python: String,
    /// Where the application writes its own logs.
    pub logs_dir: String,
    /// API host, for display and health probes.
    pub api_host: String,
    /// API port, for display, health probes and the launch command.
    pub api_port: u16,
    /// Name the server announces, for display only.
    pub served_model: String,
    /// Optional V1-style `config.json` to import paths from, once.
    pub legacy_config: String,
}

impl Default for Paths {
    fn default() -> Self {
        Self {
            vllm_root: String::new(),
            vllm_exe: String::new(),
            python_exe: String::new(),
            model_checkpoint: String::new(),
            launch_config: String::new(),
            chat_template: String::new(),
            blackout_engine: String::new(),
            blackout_python: String::new(),
            logs_dir: String::new(),
            api_host: "127.0.0.1".to_string(),
            api_port: 8000,
            served_model: String::new(),
            legacy_config: String::new(),
        }
    }
}

/// The fields the interface may edit, with the validation each one needs.
pub const FIELDS: &[(&str, Kind)] = &[
    ("vllm_root", Kind::Dir),
    ("vllm_exe", Kind::Exe),
    ("python_exe", Kind::Exe),
    ("model_checkpoint", Kind::Dir),
    ("launch_config", Kind::File),
    ("chat_template", Kind::File),
    ("blackout_engine", Kind::File),
    ("blackout_python", Kind::Exe),
    ("logs_dir", Kind::OptionalDir),
    ("legacy_config", Kind::OptionalFile),
];

impl Paths {
    pub fn kind_of(field: &str) -> Option<Kind> {
        FIELDS
            .iter()
            .find(|(name, _)| *name == field)
            .map(|(_, kind)| *kind)
    }

    /// Display name for the served model: the configured label, else the
    /// checkpoint's own folder name, else a neutral placeholder.
    pub fn served_name(&self) -> String {
        if !self.served_model.trim().is_empty() {
            return self.served_model.trim().to_string();
        }
        Path::new(&self.model_checkpoint)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "local model".to_string())
    }

    pub fn host(&self) -> String {
        let host = self.api_host.trim();
        if host.is_empty() {
            "127.0.0.1".to_string()
        } else {
            host.to_string()
        }
    }

    pub fn port(&self) -> u16 {
        // The QA hook outranks configuration so a lifecycle run can never touch
        // the port a real server would use.
        if let Ok(raw) = std::env::var("V2_TEST_PORT") {
            if let Ok(port) = raw.parse::<u16>() {
                return port;
            }
        }
        if self.api_port == 0 {
            8000
        } else {
            self.api_port
        }
    }

    pub fn endpoint(&self) -> String {
        format!("http://{}:{}/v1", self.host(), self.port())
    }

    /// The four values `vllm serve` cannot start without.
    pub fn launch_missing(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        for (field, label) in [
            ("vllm_exe", "vLLM launcher"),
            ("model_checkpoint", "model checkpoint"),
            ("launch_config", "serve configuration"),
            ("chat_template", "chat template"),
        ] {
            if self.field(field).trim().is_empty() {
                missing.push(label);
            }
        }
        missing
    }

    pub fn blackout_missing(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if self.blackout_engine.trim().is_empty() {
            missing.push("blackout engine");
        }
        if self.blackout_python.trim().is_empty() {
            missing.push("blackout interpreter");
        }
        missing
    }

    /// True when every launch path exists *right now*. Used for `can_start`, so a
    /// folder that was moved makes Start refuse instead of failing half way.
    pub fn launch_ready(&self) -> bool {
        if !self.launch_missing().is_empty() {
            return false;
        }
        check(&self.vllm_exe, Kind::Exe).is_ok()
            && check(&self.model_checkpoint, Kind::Dir).is_ok()
            && check(&self.launch_config, Kind::File).is_ok()
            && check(&self.chat_template, Kind::File).is_ok()
    }

    pub fn blackout_ready(&self) -> bool {
        if !self.blackout_missing().is_empty() {
            return false;
        }
        check(&self.blackout_engine, Kind::File).is_ok()
            && check(&self.blackout_python, Kind::Exe).is_ok()
    }

    pub fn field(&self, name: &str) -> &str {
        match name {
            "vllm_root" => &self.vllm_root,
            "vllm_exe" => &self.vllm_exe,
            "python_exe" => &self.python_exe,
            "model_checkpoint" => &self.model_checkpoint,
            "launch_config" => &self.launch_config,
            "chat_template" => &self.chat_template,
            "blackout_engine" => &self.blackout_engine,
            "blackout_python" => &self.blackout_python,
            "logs_dir" => &self.logs_dir,
            "legacy_config" => &self.legacy_config,
            _ => "",
        }
    }
}

/// Characters that have no business in a filesystem path, plus the ones that
/// would make a value look like a command line if anybody ever logged or pasted
/// it into one.
const FORBIDDEN: &[char] = &[
    '\0', '\n', '\r', '"', '\'', '`', '$', '&', '|', ';', '<', '>', '*', '?',
];

/// Validate one configured path.
///
/// Returns the normalised path on success. The message on failure is written for
/// the person who typed it, because it is shown verbatim next to the field.
pub fn check(raw: &str, kind: Kind) -> Result<PathBuf, String> {
    let text = raw.trim();
    if text.is_empty() {
        if kind.optional() {
            return Err(String::new());
        }
        return Err(format!("a {} is required", kind.label()));
    }
    if text.chars().any(|c| FORBIDDEN.contains(&c)) {
        return Err("contains characters that are not valid in a path".to_string());
    }
    if text.len() > 512 {
        return Err("path is longer than 512 characters".to_string());
    }

    let path = PathBuf::from(text);
    if !path.is_absolute() {
        return Err("must be a full path, such as C:\\folder\\file".to_string());
    }
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            return Err("must not contain .. (parent folder segments)".to_string());
        }
    }
    if let Some(stem) = path.file_stem() {
        let upper = stem.to_string_lossy().to_uppercase();
        let reserved = ["CON", "PRN", "AUX", "NUL"]
            .contains(&upper.as_str())
            || ((upper.starts_with("COM") || upper.starts_with("LPT"))
                && upper.len() == 4
                && upper.chars().last().is_some_and(|c| c.is_ascii_digit()));
        if reserved {
            return Err("is a reserved Windows device name".to_string());
        }
    }

    match std::fs::metadata(&path) {
        Ok(meta) => {
            if kind.wants_dir() && !meta.is_dir() {
                return Err("is a file, but a folder was expected".to_string());
            }
            if !kind.wants_dir() && meta.is_dir() {
                return Err("is a folder, but a file was expected".to_string());
            }
        }
        Err(_) => {
            if kind.optional() {
                return Err(String::new());
            }
            return Err("does not exist".to_string());
        }
    }

    if matches!(kind, Kind::Exe) && !has_exe_extension(&path) {
        return Err("must point at an .exe".to_string());
    }
    Ok(path)
}

fn has_exe_extension(path: &Path) -> bool {
    path.extension()
        .map(|ext| ext.eq_ignore_ascii_case("exe"))
        .unwrap_or(false)
}

/// One thing discovery found, offered to the operator. Never applied silently.
#[derive(Serialize, Clone, Debug)]
pub struct Candidate {
    pub field: String,
    pub path: String,
    pub note: String,
}

/// Bounded search for installations, so first run can offer what already exists
/// on this machine instead of asking someone to type eleven paths.
///
/// The walk is deliberately shallow and cheap: a fixed allowlist of roots, two
/// levels deep, a hard cap on entries, and the usual offenders skipped. It never
/// reads file contents beyond `build-info.json`.
pub fn discover() -> Vec<Candidate> {
    let mut found: Vec<Candidate> = Vec::new();

    // 1. Anything the environment already knows about.
    for (var, field, note) in [
        ("V2_VLLM_ROOT", "vllm_root", "from V2_VLLM_ROOT"),
        ("VIRTUAL_ENV", "vllm_root", "active virtual environment"),
        ("VLLM_ROOT", "vllm_root", "from VLLM_ROOT"),
    ] {
        if let Ok(value) = std::env::var(var) {
            if Path::new(&value).is_dir() {
                push_candidate(&mut found, field, PathBuf::from(&value), note);
            }
        }
    }
    if let Ok(hf) = std::env::var("HF_HOME") {
        if Path::new(&hf).is_dir() {
            push_candidate(
                &mut found,
                "model_checkpoint",
                PathBuf::from(hf),
                "HF_HOME (cache root)",
            );
        }
    }

    // 2. Conventional layout under the roots worth looking at.
    let mut roots: Vec<PathBuf> = Vec::new();
    for var in ["USERPROFILE", "LOCALAPPDATA"] {
        if let Ok(value) = std::env::var(var) {
            roots.push(PathBuf::from(value));
        }
    }
    for drive in ["C:", "D:"] {
        roots.push(PathBuf::from(format!("{drive}\\")));
    }

    let mut visited = 0usize;
    for root in &roots {
        scan_root(root, 2, &mut found, &mut visited);
    }

    // 3. A model checkpoint has no convention, so offer folders that look like a
    //    weights directory: a config.json beside safetensors.
    for root in &roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten().take(400) {
            if found.iter().any(|c| c.field == "model_checkpoint") {
                break;
            }
            let path = entry.path();
            if !path.is_dir() || !path.join("config.json").is_file() {
                continue;
            }
            let weights = path.join("model.safetensors").is_file()
                || path.join("model.safetensors.index.json").is_file();
            if weights {
                push_candidate(
                    &mut found,
                    "model_checkpoint",
                    path,
                    "folder with config.json and safetensors weights",
                );
            }
        }
    }

    found.retain(|candidate| match Paths::kind_of(&candidate.field) {
        Some(kind) => check(&candidate.path, kind).is_ok(),
        None => false,
    });
    found.truncate(40);
    found
}

fn push_candidate(found: &mut Vec<Candidate>, field: &str, path: PathBuf, note: &str) {
    let text = path.to_string_lossy().to_string();
    if found
        .iter()
        .any(|candidate| candidate.field == field && candidate.path.eq_ignore_ascii_case(&text))
    {
        return;
    }
    found.push(Candidate {
        field: field.to_string(),
        path: text,
        note: note.to_string(),
    });
}

/// Bounded walk of one root. Entries are capped so a first run can never spend
/// more than a moment looking around.
fn scan_root(root: &Path, depth: usize, found: &mut Vec<Candidate>, visited: &mut usize) {
    if !root.is_dir() {
        return;
    }
    const MAX_ENTRIES: usize = 4000;
    let mut queue: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    while let Some((dir, level)) = queue.pop() {
        if *visited > MAX_ENTRIES {
            return;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if *visited > MAX_ENTRIES {
                return;
            }
            *visited += 1;
            let name = entry.file_name().to_string_lossy().to_lowercase();
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if matches!(
                name.as_str(),
                "node_modules" | ".git" | "target" | "windows" | "$recycle.bin" | "appdata"
            ) {
                continue;
            }
            // A Python environment: the launcher and interpreter are siblings.
            if name == "scripts" {
                let exe = path.join("vllm.exe");
                if exe.is_file() {
                    push_candidate(found, "vllm_exe", exe, &format!("found in {}", dir.display()));
                }
                let python = path.join("python.exe");
                if python.is_file() && !found.iter().any(|c| c.field == "python_exe") {
                    push_candidate(
                        found,
                        "python_exe",
                        python,
                        &format!("found in {}", dir.display()),
                    );
                }
                continue;
            }
            if path.join("build-info.json").is_file() {
                push_candidate(
                    found,
                    "vllm_root",
                    path.clone(),
                    "native-vLLM installation (has build-info.json)",
                );
            }
            let script = path.join("blackout.py");
            if script.is_file() {
                push_candidate(
                    found,
                    "blackout_engine",
                    script,
                    &format!("found in {}", path.display()),
                );
            }
            if level < depth {
                queue.push((path, level + 1));
            }
        }
    }
}

/// Import paths from a V1-shaped `config.json`.
///
/// The keys are V1's public schema, not this machine's values: whatever is
/// imported is what the file says, and every imported field is validated exactly
/// like typed input.
pub fn import_legacy(config_path: &Path) -> Result<Paths, String> {
    let text = std::fs::read_to_string(config_path)
        .map_err(|error| format!("cannot read {}: {error}", config_path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| format!("not valid JSON: {error}"))?;

    let get = |section: &str, key: &str| -> String {
        value
            .get(section)
            .and_then(|s| s.get(key))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };

    let mut paths = Paths::default();
    paths.vllm_exe = get("server", "vllm_exe");
    paths.model_checkpoint = get("server", "model_path");
    paths.launch_config = get("server", "config_path");
    paths.chat_template = get("server", "chat_template");
    paths.served_model = get("server", "served_model");
    paths.blackout_python = get("blackout", "python");
    paths.blackout_engine = get("blackout", "script");
    if let Some(port) = value
        .get("server")
        .and_then(|s| s.get("port"))
        .and_then(|p| p.as_u64())
    {
        if port > 0 && port < 65_536 {
            paths.api_port = port as u16;
        }
    }
    if let Some(host) = value
        .get("server")
        .and_then(|s| s.get("host"))
        .and_then(|h| h.as_str())
    {
        if !host.trim().is_empty() {
            paths.api_host = host.trim().to_string();
        }
    }
    // A launcher inside `<root>\...\Scripts\vllm.exe` implies the root two levels
    // up, which is what the version reader wants.
    if paths.vllm_root.is_empty() {
        if let Some(scripts) = Path::new(&paths.vllm_exe).parent() {
            let guess = scripts.parent().map(|env| {
                if env.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(false)
                {
                    env.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| env.to_path_buf())
                } else {
                    env.to_path_buf()
                }
            });
            if let Some(guess) = guess {
                paths.vllm_root = guess.to_string_lossy().to_string();
            }
        }
    }

    if paths.vllm_exe.is_empty() && paths.model_checkpoint.is_empty() && paths.launch_config.is_empty()
    {
        return Err("no recognisable server paths in that file".to_string());
    }
    Ok(paths)
}

// ------------------------------------------------------------------- unit tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_required_paths() {
        assert!(check("", Kind::Exe).is_err());
        assert!(check("   ", Kind::Dir).is_err());
    }

    #[test]
    fn optional_fields_may_be_blank() {
        // Blank optional values are not a failure, they are simply unset.
        assert_eq!(check("", Kind::OptionalDir), Err(String::new()));
    }

    #[test]
    fn rejects_relative_paths() {
        let error = check("models/qwen", Kind::Dir).unwrap_err();
        assert!(error.contains("full path"), "{error}");
    }

    #[test]
    fn rejects_parent_traversal() {
        let error = check(r"C:\models\..\windows\system32\cmd.exe", Kind::Exe).unwrap_err();
        assert!(error.contains(".."), "{error}");
    }

    #[test]
    fn rejects_shell_metacharacters() {
        for bad in [
            r"C:\a&b\vllm.exe",
            r"C:\a|b\vllm.exe",
            r"C:\a;b\vllm.exe",
            "C:\\a`b\\vllm.exe",
            r"C:\a$(calc)\vllm.exe",
            "C:\\a\0b",
        ] {
            let error = check(bad, Kind::Exe).unwrap_err();
            assert!(!error.is_empty(), "{bad} was accepted");
        }
    }

    #[test]
    fn rejects_reserved_device_names() {
        let error = check(r"C:\temp\CON.txt", Kind::File).unwrap_err();
        assert!(error.contains("reserved"), "{error}");
    }

    #[test]
    fn rejects_missing_files_and_wrong_kind() {
        let missing = check(r"C:\definitely\missing\thing.exe", Kind::Exe).unwrap_err();
        assert!(missing.contains("does not exist"), "{missing}");

        let dir = std::env::temp_dir();
        let error = check(&dir.to_string_lossy(), Kind::Exe).unwrap_err();
        assert!(error.contains("folder"), "{error}");
    }

    #[test]
    fn accepts_an_existing_file() {
        let exe = std::env::current_exe().expect("test binary path");
        let checked = check(&exe.to_string_lossy(), Kind::Exe).expect("own executable validates");
        assert!(checked.is_absolute());
    }

    #[test]
    fn endpoints_and_names_fall_back_sensibly() {
        let mut paths = Paths::default();
        assert_eq!(paths.endpoint(), "http://127.0.0.1:8000/v1");
        assert_eq!(paths.served_name(), "local model");

        paths.model_checkpoint = r"C:\models\Qwen3.8-Flash-Next-FP4-Local".to_string();
        assert_eq!(paths.served_name(), "Qwen3.8-Flash-Next-FP4-Local");

        paths.api_port = 8011;
        assert_eq!(paths.endpoint(), "http://127.0.0.1:8011/v1");
    }

    #[test]
    fn launch_readiness_needs_all_four_paths() {
        let paths = Paths::default();
        assert!(!paths.launch_ready());
        assert_eq!(paths.launch_missing().len(), 4);
        assert_eq!(paths.blackout_missing().len(), 2);
    }

    #[test]
    fn imports_a_v1_shaped_config() {
        let dir = std::env::temp_dir().join("v2-legacy-import-test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("config.json");
        std::fs::write(
            &file,
            r#"{
              "server": {
                "port": 8011,
                "host": "127.0.0.1",
                "model_path": "C:\\models\\demo",
                "served_model": "demo/served",
                "vllm_exe": "C:\\envs\\demo\\.venv\\Scripts\\vllm.exe",
                "config_path": "C:\\cfg\\serve.yaml",
                "chat_template": "C:\\cfg\\chat.jinja"
              },
              "blackout": {
                "python": "C:\\envs\\demo\\.venv\\Scripts\\pythonw.exe",
                "script": "C:\\blackout\\blackout.py"
              }
            }"#,
        )
        .unwrap();

        let paths = import_legacy(&file).expect("import succeeds");
        assert_eq!(paths.api_port, 8011);
        assert_eq!(paths.served_model, "demo/served");
        assert_eq!(paths.model_checkpoint, r"C:\models\demo");
        // The root is inferred from `<root>\.venv\Scripts\vllm.exe`.
        assert_eq!(paths.vllm_root, r"C:\envs\demo");
        assert_eq!(paths.blackout_engine, r"C:\blackout\blackout.py");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_a_config_without_server_paths() {
        let dir = std::env::temp_dir().join("v2-legacy-import-empty");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("config.json");
        std::fs::write(&file, r#"{"server":{"port":8000}}"#).unwrap();
        assert!(import_legacy(&file).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discovery_finds_a_conventional_installation_layout() {
        // A synthetic tree shaped the way installs usually look: a Python env with
        // the launcher and interpreter, a build record, and the engine script.
        let root = std::env::temp_dir().join("v2-discover-test");
        let _ = std::fs::remove_dir_all(&root);
        let env_scripts = root.join("native-vLLM").join(".venv").join("Scripts");
        std::fs::create_dir_all(&env_scripts).unwrap();
        std::fs::write(env_scripts.join("vllm.exe"), b"").unwrap();
        std::fs::write(env_scripts.join("python.exe"), b"").unwrap();
        std::fs::write(root.join("native-vLLM").join("build-info.json"), b"{}").unwrap();
        let blackout = root.join("RTX-Blackout");
        std::fs::create_dir_all(&blackout).unwrap();
        std::fs::write(blackout.join("blackout.py"), b"").unwrap();

        let mut found: Vec<Candidate> = Vec::new();
        let mut visited = 0usize;
        scan_root(&root, 2, &mut found, &mut visited);

        let has = |field: &str, suffix: &str| {
            found
                .iter()
                .any(|c| c.field == field && c.path.ends_with(suffix))
        };
        assert!(has("vllm_exe", "vllm.exe"), "{found:?}");
        assert!(has("python_exe", "python.exe"), "{found:?}");
        assert!(has("vllm_root", "native-vLLM"), "{found:?}");
        assert!(has("blackout_engine", "blackout.py"), "{found:?}");
        assert!(visited > 0);

        // Discovery never proposes the same field and path twice.
        let mut seen: Vec<(String, String)> = Vec::new();
        for candidate in &found {
            let key = (candidate.field.clone(), candidate.path.to_lowercase());
            assert!(!seen.contains(&key), "duplicate candidate {key:?}");
            seen.push(key);
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn discovery_ignores_build_and_dependency_folders() {
        let root = std::env::temp_dir().join("v2-discover-skip-test");
        let _ = std::fs::remove_dir_all(&root);
        for skip in ["node_modules", "target", ".git"] {
            let dir = root.join(skip).join("Scripts");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("vllm.exe"), b"").unwrap();
        }
        let mut found: Vec<Candidate> = Vec::new();
        let mut visited = 0usize;
        scan_root(&root, 3, &mut found, &mut visited);
        assert!(found.is_empty(), "found {found:?}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
