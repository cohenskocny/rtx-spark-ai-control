/** Shapes returned by the Rust backend (`invoke('snapshot')` and the actions). */

export interface GpuInfo {
  available: boolean;
  name: string;
  util_pct: number | null;
  mem_used_mib: number | null;
  mem_total_mib: number | null;
  temp_c: number | null;
  power_w: number | null;
  clock_mhz: number | null;
  throttle: string;
  error: string | null;
  collected_ms: number;
}

export interface ModelInfo {
  served_name: string;
  checkpoint: string;
  checkpoint_exists: boolean;
  endpoint: string;
  config_path: string;
  chat_template: string;
  dtype: string;
  kv_cache_dtype: string;
  max_model_len: number | null;
  gpu_memory_utilization: number | null;
  mtp_depth: number | null;
  max_num_seqs: number | null;
  prefix_caching: boolean;
}

/** `ready` · `loading` · `starting` · `stopping` · `stopped` · `external` · `error` */
export interface ServerView {
  state: string;
  detail: string;
  /** launching · weights · serving · idle */
  stage: string;
  progress_pct: number | null;
  owned: boolean;
  adopted: boolean;
  pid: number | null;
  listening: boolean;
  process_name: string | null;
  uptime_s: number | null;
  health_ok: boolean;
  latency_ms: number | null;
  health_error: string | null;
  health_failures: number;
  models: string[];
  endpoint: string;
  port: number;
  started_ms: number | null;
  log_tail: string[];
  last_error: string;
  paths_ok: boolean;
  can_start: boolean;
  can_stop: boolean;
  can_restart: boolean;
  can_adopt: boolean;
}

/** `off` · `armed` · `active` */
export interface BlackoutView {
  running: boolean;
  owned: boolean;
  mode: string;
  active: boolean;
  phase: string;
  display_held: boolean;
  display_request: boolean;
  locked: boolean;
  idle_s: number | null;
  idle_remaining_s: number | null;
  updated_utc: string;
  cover: string;
  pid: number | null;
  state_path: string;
  stop_file_present: boolean;
  error: string;
  since_s: number | null;
  idle_threshold_s: number;
}

/** disabled · paused · waiting-server · armed · starting */
export interface AutomationView {
  phase: string;
  reason: string;
  idle_s: number;
  idle_remaining_s: number;
  armed: boolean;
  server_ok: boolean;
  suspended: boolean;
  follow_server: boolean;
  enabled: boolean;
  grace_s: number;
  hysteresis_s: number;
  poll_s: number;
}

export interface SettingsView {
  blackout_enabled: boolean;
  idle_seconds: number;
  follow_server: boolean;
  cover: string;
  grace_s: number;
  hysteresis_s: number;
  poll_s: number;
  stop_timeout_s: number;
}

export type EventKind = "info" | "ok" | "warn" | "error" | "blackout" | "log";

export interface LogEvent {
  id: number;
  at: number;
  kind: EventKind;
  text: string;
  repeat: number;
}

export interface ActionEcho {
  ok: boolean;
  message: string;
  at: number;
}

export interface Snapshot {
  seq: number;
  collected_ms: number;
  collect_duration_ms: number;
  server_uptime_s: number;
  host: string;
  model: ModelInfo;
  gpu: GpuInfo;
  server: ServerView;
  blackout: BlackoutView;
  automation: AutomationView;
  settings: SettingsView;
  events: LogEvent[];
  notes: string[];
  last_action: ActionEcho | null;
}

export interface ActionResult {
  ok: boolean;
  message: string;
}

export interface ReadyReport {
  startup_ms: number;
  arch: string;
  version: string;
}

export interface VisibilityEvent {
  minimized: boolean;
  visible: boolean;
  focused: boolean;
}
