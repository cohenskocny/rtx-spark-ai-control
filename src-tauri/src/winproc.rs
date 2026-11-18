//! Native Windows process/socket introspection.
//!
//! Two questions only: "which pid is listening on this port?" and "what is that
//! process, and when did it start?". Both are answered through `windows-sys`
//! rather than spawned tools so that a poll can never flash a console window or
//! depend on the locale of `netstat` output.

#![cfg(windows)]

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, FILETIME};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID,
};

/// `TCP_TABLE_OWNER_PID_LISTENER` from iphlpapi.h. windows-sys 0.59 exposes the
/// enum type but not this variant, so it is declared here.
const TCP_TABLE_OWNER_PID_LISTENING: i32 = 3;
use windows_sys::Win32::Networking::WinSock::AF_INET;
use windows_sys::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// One listening socket owner.
pub struct Listener {
    pub pid: u32,
}

fn as_owner_table(buf: &[u8]) -> Option<&MIB_TCPTABLE_OWNER_PID> {
    if buf.len() < std::mem::size_of::<MIB_TCPTABLE_OWNER_PID>() {
        return None;
    }
    Some(unsafe { &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID) })
}

/// All TCP listeners for the given port, IPv4.
pub fn listeners_on(port: u16) -> Vec<Listener> {
    let mut size: u32 = 0;
    unsafe {
        GetExtendedTcpTable(
            std::ptr::null_mut(),
            &mut size,
            0,
            AF_INET as u32,
            TCP_TABLE_OWNER_PID_LISTENING,
            0,
        );
    }
    if size == 0 {
        return Vec::new();
    }

    for _ in 0..3 {
        let mut buf = vec![0u8; size as usize];
        let mut len = size;
        let rc = unsafe {
            GetExtendedTcpTable(
                buf.as_mut_ptr() as *mut _,
                &mut len,
                0,
                AF_INET as u32,
                TCP_TABLE_OWNER_PID_LISTENING,
                0,
            )
        };
        if rc == ERROR_INSUFFICIENT_BUFFER {
            size = len.max(size * 2);
            continue;
        }
        if rc != 0 {
            return Vec::new();
        }
        let Some(table) = as_owner_table(&buf) else {
            return Vec::new();
        };
        let count = table.dwNumEntries as usize;
        let rows: &[MIB_TCPROW_OWNER_PID] =
            unsafe { std::slice::from_raw_parts(table.table.as_ptr(), count) };
        return rows
            .iter()
            .filter_map(|row| {
                let local = u16::from_be((row.dwLocalPort & 0xffff) as u16);
                (local == port).then_some(Listener {
                    pid: row.dwOwningPid,
                })
            })
            .take(8)
            .collect();
    }
    Vec::new()
}

/// Image name (e.g. `python.exe`) and creation time in 100 ns units since 1601.
pub struct ProcessFacts {
    pub name: String,
    pub created_ticks: u64,
}

fn filetime_ticks(ft: &FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}

pub fn process_facts(pid: u32) -> Option<ProcessFacts> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut creation = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut exit = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut kernel = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut user = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let ok = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user);

        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let named = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);

        if ok == 0 || named == 0 {
            return None;
        }
        let path = OsString::from_wide(&buf[..len as usize])
            .to_string_lossy()
            .to_string();
        let name = path
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(&path)
            .to_string();
        Some(ProcessFacts {
            name,
            created_ticks: filetime_ticks(&creation),
        })
    }
}

// ---------------------------------------------------------------- command line

#[link(name = "ntdll", kind = "raw-dylib")]
extern "system" {
    fn NtQueryInformationProcess(
        process: *mut std::ffi::c_void,
        class: u32,
        info: *mut std::ffi::c_void,
        len: u32,
        returned: *mut u32,
    ) -> i32;
}

#[link(name = "kernel32", kind = "raw-dylib")]
extern "system" {
    fn ReadProcessMemory(
        process: *mut std::ffi::c_void,
        base: *const std::ffi::c_void,
        buffer: *mut std::ffi::c_void,
        size: usize,
        read: *mut usize,
    ) -> i32;
}

#[repr(C)]
struct ProcessBasicInformation {
    exit_status: i32,
    peb_base_address: *mut std::ffi::c_void,
    affinity_mask: usize,
    base_priority: i32,
    unique_process_id: usize,
    inherited_from_unique_process_id: usize,
}

const PROCESS_VM_READ: u32 = 0x0010;
/// PEB.ProcessParameters, RTL_USER_PROCESS_PARAMETERS.CommandLine: the same
/// offsets on x64 and ARM64, the only two architectures this ships for.
const PEB_PROCESS_PARAMETERS: usize = 0x20;
const RTL_USER_PROCESS_PARAMETERS_COMMAND_LINE: usize = 0x70;

fn read_bytes(handle: *mut std::ffi::c_void, address: usize, out: &mut [u8]) -> bool {
    if address == 0 {
        return false;
    }
    let mut read = 0usize;
    let ok = unsafe {
        ReadProcessMemory(
            handle,
            address as *const std::ffi::c_void,
            out.as_mut_ptr() as *mut std::ffi::c_void,
            out.len(),
            &mut read,
        )
    };
    ok != 0 && read == out.len()
}

/// The full command line of another process, or `None` when it cannot be read.
///
/// Used only to keep process ownership honest: adoption must never widen to a
/// parent that is not part of the server, whatever its executable is called.
pub fn process_cmdline(pid: u32) -> Option<String> {
    if pid == 0 || pid == std::process::id() {
        return None;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, 0, pid);
        if handle.is_null() {
            return None;
        }
        let cmdline = cmdline_from_handle(handle);
        CloseHandle(handle);
        cmdline
    }
}

unsafe fn cmdline_from_handle(handle: *mut std::ffi::c_void) -> Option<String> {
    let mut info = ProcessBasicInformation {
        exit_status: 0,
        peb_base_address: std::ptr::null_mut(),
        affinity_mask: 0,
        base_priority: 0,
        unique_process_id: 0,
        inherited_from_unique_process_id: 0,
    };
    let mut returned = 0u32;
    let status = NtQueryInformationProcess(
        handle,
        0,
        &mut info as *mut ProcessBasicInformation as *mut std::ffi::c_void,
        std::mem::size_of::<ProcessBasicInformation>() as u32,
        &mut returned,
    );
    if status != 0 || info.peb_base_address.is_null() {
        return None;
    }

    let mut pointer = [0u8; 8];
    if !read_bytes(
        handle,
        info.peb_base_address as usize + PEB_PROCESS_PARAMETERS,
        &mut pointer,
    ) {
        return None;
    }
    let parameters = usize::from_le_bytes(pointer);
    if parameters == 0 {
        return None;
    }

    // UNICODE_STRING: length, maximum length, padding, then the buffer pointer.
    let mut unicode = [0u8; 16];
    if !read_bytes(
        handle,
        parameters + RTL_USER_PROCESS_PARAMETERS_COMMAND_LINE,
        &mut unicode,
    ) {
        return None;
    }
    let length = u16::from_le_bytes([unicode[0], unicode[1]]) as usize;
    let buffer = usize::from_le_bytes(unicode[8..16].try_into().ok()?);
    if length == 0 || length > 32_768 || length % 2 != 0 || buffer == 0 {
        return None;
    }

    let mut raw = vec![0u8; length];
    if !read_bytes(handle, buffer, &mut raw) {
        return None;
    }
    let units: Vec<u16> = raw
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let text = String::from_utf16_lossy(&units);
    let text = text.trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Seconds since the process was created (best effort).
pub fn process_uptime_s(created_ticks: u64) -> Option<f64> {
    if created_ticks == 0 {
        return None;
    }
    let now_ticks = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos() as u64
        / 100;
    // 11644473600 s between 1601-01-01 and 1970-01-01, in 100 ns units.
    let epoch_offset = 116_444_736_000_000_000u64;
    let now = now_ticks + epoch_offset;
    Some(now.saturating_sub(created_ticks) as f64 / 1e7)
}

// ------------------------------------------------------------------- process tree

/// One row of the system process list: enough to walk parent/child relations.
pub struct ProcEntry {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
}

/// Whole process table via Toolhelp. One call, a few milliseconds, no console.
pub fn snapshot_processes() -> Vec<ProcEntry> {
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut out = Vec::with_capacity(320);
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot.is_null() {
            return out;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snapshot, &mut entry);
        while ok != 0 {
            let end = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            out.push(ProcEntry {
                pid: entry.th32ProcessID,
                parent_pid: entry.th32ParentProcessID,
                name: String::from_utf16_lossy(&entry.szExeFile[..end]),
            });
            ok = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
    }
    out
}

/// `root` first, then its children, depth first.
pub fn tree_pids(root: u32) -> Vec<u32> {
    let table = snapshot_processes();
    let mut order = Vec::new();
    let mut frontier = vec![root];
    while let Some(pid) = frontier.pop() {
        if pid == 0 || order.contains(&pid) {
            continue;
        }
        order.push(pid);
        for entry in table.iter().filter(|e| e.parent_pid == pid) {
            frontier.push(entry.pid);
        }
    }
    order
}

/// Parents of `pid`, nearest first, stopping at a shell or at this process.
pub fn ancestors(pid: u32) -> Vec<ProcEntry> {
    let table = snapshot_processes();
    let me = std::process::id();
    let mut out = Vec::new();
    let mut current = pid;
    for _ in 0..8 {
        let Some(entry) = table.iter().find(|e| e.pid == current) else {
            break;
        };
        let parent = entry.parent_pid;
        if parent == 0 || parent == current || parent == me {
            break;
        }
        let Some(next) = table.iter().find(|e| e.pid == parent) else {
            break;
        };
        if next.pid == me || next.parent_pid == me {
            // Never walk into this application.
            break;
        }
        out.push(ProcEntry {
            pid: next.pid,
            parent_pid: next.parent_pid,
            name: next.name.clone(),
        });
        current = parent;
    }
    out
}

fn terminate(pid: u32) -> bool {
    use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

    if pid == 0 || pid == std::process::id() {
        return false;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if handle.is_null() {
            return false;
        }
        let ok = TerminateProcess(handle, 1) != 0;
        CloseHandle(handle);
        ok
    }
}

/// Terminate `root` and everything below it, children first.
///
/// Returns true when nothing from that tree is left alive. This is the same
/// contract V1's `terminate_tree` provides: the caller must already have proven
/// ownership, because nothing here checks what the processes are.
pub fn terminate_tree(root: u32, timeout_ms: u64) -> bool {
    let pids = tree_pids(root);
    for pid in pids.iter().rev() {
        terminate(*pid);
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    while std::time::Instant::now() < deadline {
        if process_facts(root).is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(120));
    }
    process_facts(root).is_none() && tree_pids(root).len() <= 1
}

/// True when the recorded process is still the one running now.
pub fn same_process(pid: u32, created_ticks: u64) -> bool {
    match process_facts(pid) {
        Some(facts) => facts.created_ticks == created_ticks,
        None => false,
    }
}

// ------------------------------------------------------------------ idle / input

/// Raw last-input tick: the same source the blackout engine counts idle from,
/// so the two never disagree about when the machine was last touched. Change
/// detection only needs 32 bits of it.
pub fn input_tick() -> u32 {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    unsafe {
        let mut info: LASTINPUTINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<LASTINPUTINFO>() as u32;
        if GetLastInputInfo(&mut info) == 0 {
            return 0;
        }
        info.dwTime
    }
}
