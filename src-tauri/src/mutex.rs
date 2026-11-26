use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[cfg(windows)]
use windows::{
    core::w,
    Win32::{
        Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            Threading::{
                CreateEventW, CreateMutexW, OpenProcess, TerminateProcess, PROCESS_TERMINATE,
            },
        },
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MutexStatus {
    pub active: bool,
    pub event_acquired: bool,
    pub mutex_acquired: bool,
    pub already_existed: bool,
    pub roblox_running_at_startup: bool,
    pub roblox_running_now: bool,
    pub auto_closed_count: usize,
    pub error: Option<String>,
}

struct OwnedHandle(isize);

// Win32 kernel object handles are process-wide and safe to send/share across threads.
unsafe impl Send for OwnedHandle {}
unsafe impl Sync for OwnedHandle {}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        #[cfg(windows)]
        if self.0 != 0 {
            unsafe {
                let _ = CloseHandle(HANDLE(self.0 as *mut std::ffi::c_void));
            }
        }
    }
}

struct MutexInner {
    _event_handle: Option<OwnedHandle>,
    _mutex_handle: Option<OwnedHandle>,
    event_acquired: bool,
    mutex_acquired: bool,
    already_existed: bool,
    roblox_running_at_startup: bool,
    auto_closed_count: usize,
    error: Option<String>,
}

pub struct MutexManager {
    inner: Mutex<MutexInner>,
}

impl MutexManager {
    pub fn new() -> Self {
        let roblox_running_at_startup = is_roblox_process_running();
        // Automatically close any pre-existing Roblox instances on startup so RoMulti
        // can cleanly take ownership of ROBLOX_singletonEvent & ROBLOX_singletonMutex.
        let mut auto_closed_count = 0usize;
        if roblox_running_at_startup {
            auto_closed_count = terminate_all_roblox_processes_and_wait();
        }

        let inner = acquire_handles_with_retry(roblox_running_at_startup, auto_closed_count);
        Self {
            inner: Mutex::new(inner),
        }
    }

    pub fn is_active(&self) -> bool {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.event_acquired && guard.mutex_acquired && !guard.already_existed
    }

    pub fn status(&self) -> MutexStatus {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let roblox_running_now = is_roblox_process_running();
        let active = guard.event_acquired && guard.mutex_acquired && !guard.already_existed;
        MutexStatus {
            active,
            event_acquired: guard.event_acquired,
            mutex_acquired: guard.mutex_acquired,
            already_existed: guard.already_existed,
            roblox_running_at_startup: guard.roblox_running_at_startup,
            roblox_running_now,
            auto_closed_count: guard.auto_closed_count,
            error: guard.error.clone(),
        }
    }

    /// Closes any conflicting Roblox processes, releases current handles, and cleanly
    /// re-acquires ROBLOX_singletonEvent and ROBLOX_singletonMutex.
    pub fn reinitialize(&self) -> MutexStatus {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        // Drop existing handles first so Windows releases them before re-creating.
        guard._event_handle = None;
        guard._mutex_handle = None;

        let closed = terminate_all_roblox_processes_and_wait();
        *guard = acquire_handles_with_retry(false, closed);

        let roblox_running_now = is_roblox_process_running();
        let active = guard.event_acquired && guard.mutex_acquired && !guard.already_existed;
        MutexStatus {
            active,
            event_acquired: guard.event_acquired,
            mutex_acquired: guard.mutex_acquired,
            already_existed: guard.already_existed,
            roblox_running_at_startup: guard.roblox_running_at_startup,
            roblox_running_now,
            auto_closed_count: guard.auto_closed_count,
            error: guard.error.clone(),
        }
    }

    /// Ensures the multi-instance mutex is active before launching accounts.
    /// If already cleanly held, does nothing (preserves already-launched Roblox windows).
    /// If conflicted/inactive, automatically closes conflicting Roblox processes and re-acquires.
    pub fn ensure_ready_for_launch(&self) -> MutexStatus {
        if self.is_active() {
            return self.status();
        }
        self.reinitialize()
    }
}

fn acquire_handles_with_retry(
    roblox_running_at_startup: bool,
    auto_closed_count: usize,
) -> MutexInner {
    let mut attempt = acquire_handles(roblox_running_at_startup, auto_closed_count);

    // If a closing Roblox process hadn't finished releasing its kernel handle yet, retry briefly.
    for _ in 0..5 {
        if attempt.event_acquired && attempt.mutex_acquired && !attempt.already_existed {
            break;
        }
        drop(attempt);
        std::thread::sleep(Duration::from_millis(250));
        attempt = acquire_handles(roblox_running_at_startup, auto_closed_count);
    }

    attempt
}

#[cfg(windows)]
fn acquire_handles(roblox_running_at_startup: bool, auto_closed_count: usize) -> MutexInner {
    let mut errors: Vec<String> = Vec::new();
    let mut already_existed = false;

    // 1. Create and hold ROBLOX_singletonEvent (primary multi-instance lock in modern Roblox client)
    let (event_handle, event_acquired) = unsafe {
        match CreateEventW(None, true, false, w!("ROBLOX_singletonEvent")) {
            Ok(handle) if !handle.is_invalid() => {
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    already_existed = true;
                }
                (Some(OwnedHandle(handle.0 as isize)), true)
            }
            Ok(_) => {
                errors.push("CreateEventW returned an invalid handle.".to_string());
                (None, false)
            }
            Err(err) => {
                errors.push(format!("CreateEventW failed: {err}"));
                (None, false)
            }
        }
    };

    // 2. Create and hold ROBLOX_singletonMutex (legacy/secondary multi-instance lock)
    let (mutex_handle, mutex_acquired) = unsafe {
        match CreateMutexW(None, true, w!("ROBLOX_singletonMutex")) {
            Ok(handle) if !handle.is_invalid() => {
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    already_existed = true;
                }
                (Some(OwnedHandle(handle.0 as isize)), true)
            }
            Ok(_) => {
                errors.push("CreateMutexW returned an invalid handle.".to_string());
                (None, false)
            }
            Err(err) => {
                errors.push(format!("CreateMutexW failed: {err}"));
                (None, false)
            }
        }
    };

    let error = if errors.is_empty() {
        None
    } else {
        Some(errors.join(" | "))
    };

    MutexInner {
        _event_handle: event_handle,
        _mutex_handle: mutex_handle,
        event_acquired,
        mutex_acquired,
        already_existed,
        roblox_running_at_startup,
        auto_closed_count,
        error,
    }
}

#[cfg(not(windows))]
fn acquire_handles(roblox_running_at_startup: bool, auto_closed_count: usize) -> MutexInner {
    MutexInner {
        _event_handle: None,
        _mutex_handle: None,
        event_acquired: false,
        mutex_acquired: false,
        already_existed: false,
        roblox_running_at_startup,
        auto_closed_count,
        error: Some("Multi-instance mutex is only supported on Windows.".to_string()),
    }
}

#[cfg(windows)]
fn is_target_roblox_exe(name: &str) -> bool {
    name.eq_ignore_ascii_case("RobloxPlayerBeta.exe")
        || name.eq_ignore_ascii_case("RobloxPlayerLauncher.exe")
}

#[cfg(windows)]
pub fn terminate_all_roblox_processes_and_wait() -> usize {
    let mut terminated = 0usize;

    unsafe {
        if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            if !snapshot.is_invalid() {
                let _guard = OwnedHandle(snapshot.0 as isize);
                let mut entry = PROCESSENTRY32W {
                    dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                    ..Default::default()
                };

                if Process32FirstW(snapshot, &mut entry).is_ok() {
                    loop {
                        let len = entry
                            .szExeFile
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(entry.szExeFile.len());
                        let exe_name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                        if is_target_roblox_exe(&exe_name) {
                            if let Ok(proc_handle) =
                                OpenProcess(PROCESS_TERMINATE, false, entry.th32ProcessID)
                            {
                                if !proc_handle.is_invalid() {
                                    if TerminateProcess(proc_handle, 1).is_ok() {
                                        terminated += 1;
                                    }
                                    let _ = CloseHandle(proc_handle);
                                }
                            }
                        }
                        if Process32NextW(snapshot, &mut entry).is_err() {
                            break;
                        }
                    }
                }
            }
        }
    }

    if terminated > 0 {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline && is_roblox_process_running() {
            std::thread::sleep(Duration::from_millis(100));
        }
        // Small grace delay so the Windows kernel finishes cleaning up named event/mutex handles
        std::thread::sleep(Duration::from_millis(200));
    }

    terminated
}

#[cfg(not(windows))]
pub fn terminate_all_roblox_processes_and_wait() -> usize {
    0
}

#[cfg(windows)]
pub fn is_roblox_process_running() -> bool {
    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) if !h.is_invalid() => h,
            _ => return false,
        };
        let _guard = OwnedHandle(snapshot.0 as isize);

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snapshot, &mut entry).is_err() {
            return false;
        }

        loop {
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let exe_name = String::from_utf16_lossy(&entry.szExeFile[..len]);
            if is_target_roblox_exe(&exe_name) {
                return true;
            }
            if Process32NextW(snapshot, &mut entry).is_err() {
                break;
            }
        }

        false
    }
}

#[cfg(not(windows))]
pub fn is_roblox_process_running() -> bool {
    false
}
