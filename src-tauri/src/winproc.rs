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
    pub port: u16,
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
                    port: local,
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
