use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use windows::{
    core::{BOOL, HSTRING},
    Win32::{
        Foundation::{HWND, LPARAM, RECT},
        Graphics::Gdi::{
            EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindow, GetWindowTextLengthW, GetWindowThreadProcessId, IsIconic,
            IsWindowVisible, SetForegroundWindow, SetWindowPos, SetWindowTextW, ShowWindow,
            SystemParametersInfoW, GW_OWNER, MONITORINFOF_PRIMARY, SET_WINDOW_POS_FLAGS,
            SPI_GETWORKAREA, SWP_NOACTIVATE, SWP_NOZORDER, SW_RESTORE,
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
        },
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArrangeWindowsResult {
    pub arranged_count: usize,
    pub cols: usize,
    pub rows: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    pub id: String,
    pub name: String,
    pub is_primary: bool,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Computes an optimal balanced grid (cols, rows) for `count` windows.
/// 1 -> 1x1, 2 -> 2x1, 3..=4 -> 2x2, 5..=6 -> 3x2, 7..=9 -> 3x3, etc.
pub fn compute_grid(count: usize) -> (usize, usize) {
    match count {
        0 => (0, 0),
        1 => (1, 1),
        2 => (2, 1),
        3 | 4 => (2, 2),
        5 | 6 => (3, 2),
        7..=9 => (3, 3),
        n => {
            let cols = (n as f64).sqrt().ceil() as usize;
            let rows = n.div_ceil(cols);
            (cols, rows)
        }
    }
}

/// Lists all connected display monitors and their desktop work areas (`rcWork`).
#[cfg(windows)]
pub fn list_monitors() -> Vec<MonitorInfo> {
    let mut raw_monitors: Vec<(String, bool, RECT)> = Vec::new();

    unsafe extern "system" fn monitor_enum_proc(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _lprect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let list = &mut *(lparam.0 as *mut Vec<(String, bool, RECT)>);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

        if GetMonitorInfoW(hmonitor, (&mut info.monitorInfo) as *mut MONITORINFO).as_bool() {
            let len = info
                .szDevice
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(info.szDevice.len());
            let device_id = String::from_utf16_lossy(&info.szDevice[..len]);
            let is_primary = (info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0;
            list.push((device_id, is_primary, info.monitorInfo.rcWork));
        }

        BOOL(1)
    }

    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(monitor_enum_proc),
            LPARAM((&mut raw_monitors as *mut Vec<(String, bool, RECT)>) as isize),
        );
    }

    // Sort primary monitor first, then left-to-right, top-to-bottom
    raw_monitors.sort_by_key(|(_, is_primary, rect)| (!*is_primary, rect.left, rect.top));

    raw_monitors
        .into_iter()
        .enumerate()
        .map(|(idx, (device_id, is_primary, rect))| {
            let width = (rect.right - rect.left).max(1);
            let height = (rect.bottom - rect.top).max(1);
            let id = if device_id.is_empty() {
                format!("monitor-{}", idx + 1)
            } else {
                device_id
            };
            let label = if is_primary {
                format!("Monitor {} (Primary) {}×{}", idx + 1, width, height)
            } else {
                format!("Monitor {} {}×{}", idx + 1, width, height)
            };
            MonitorInfo {
                id,
                name: label,
                is_primary,
                x: rect.left,
                y: rect.top,
                width,
                height,
            }
        })
        .collect()
}

#[cfg(not(windows))]
pub fn list_monitors() -> Vec<MonitorInfo> {
    Vec::new()
}

/// Sets the window title for the main top-level window belonging to `pid`.
#[cfg(windows)]
pub fn set_window_title_for_pid(pid: u32, title: &str) -> bool {
    if pid == 0 {
        return false;
    }
    let mut set = HashSet::new();
    set.insert(pid);
    let windows = find_top_level_windows_for_pids(&set);
    let Some(&(_, hwnd)) = windows.first() else {
        return false;
    };
    let htitle = HSTRING::from(title);
    unsafe { SetWindowTextW(hwnd, &htitle).is_ok() }
}

#[cfg(not(windows))]
pub fn set_window_title_for_pid(_pid: u32, _title: &str) -> bool {
    false
}

/// Restores (if minimized) and focuses the main top-level window belonging to `pid`.
#[cfg(windows)]
pub fn focus_window_for_pid(pid: u32) -> Result<(), String> {
    if pid == 0 {
        return Err("No active window PID for this account.".to_string());
    }
    let mut set = HashSet::new();
    set.insert(pid);
    let windows = find_top_level_windows_for_pids(&set);
    let Some(&(_, hwnd)) = windows.first() else {
        return Err(format!(
            "No visible Roblox window found for PID {pid}. It may still be loading or closed."
        ));
    };

    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let _ = SetForegroundWindow(hwnd);
    }

    Ok(())
}

#[cfg(not(windows))]
pub fn focus_window_for_pid(_pid: u32) -> Result<(), String> {
    Err("Window focus control is only supported on Windows.".to_string())
}

/// Spawns a lightweight background task that waits for a newly launched Roblox window
/// to appear and labels its title bar `Roblox — @{username}`.
pub fn spawn_window_title_setter(pid: u32, username: String) {
    if pid == 0 || username.is_empty() {
        return;
    }
    let title = format!("Roblox — @{username}");
    tauri::async_runtime::spawn(async move {
        let mut applied_once = false;
        for _ in 0..30 {
            if set_window_title_for_pid(pid, &title) {
                applied_once = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        if applied_once {
            // Re-apply once more after Roblox finishes initializing its viewport title
            tokio::time::sleep(Duration::from_secs(3)).await;
            let _ = set_window_title_for_pid(pid, &title);
        }
    });
}

/// Arranges visible top-level Roblox windows in an even grid across the selected monitor's
/// work area (or primary monitor fallback), and refreshes titles for all mapped PIDs.
#[cfg(windows)]
pub fn arrange_roblox_windows(
    tracked_pids: &[u32],
    all_roblox_pids: &HashSet<u32>,
    monitor_id: Option<&str>,
    pid_usernames: &HashMap<u32, String>,
) -> Result<ArrangeWindowsResult, String> {
    let tracked_set: HashSet<u32> = tracked_pids.iter().copied().collect();

    let mut windows = if !tracked_set.is_empty() {
        find_top_level_windows_for_pids(&tracked_set)
    } else {
        Vec::new()
    };

    // Fallback to all running RobloxPlayerBeta.exe processes if tracked PIDs yielded no windows
    if windows.is_empty() && !all_roblox_pids.is_empty() {
        windows = find_top_level_windows_for_pids(all_roblox_pids);
    }

    if windows.is_empty() {
        return Ok(ArrangeWindowsResult {
            arranged_count: 0,
            cols: 0,
            rows: 0,
        });
    }

    // Preserve tracked PID ordering where possible so windows map predictably to accounts
    windows.sort_by_key(|&(pid, _)| {
        tracked_pids
            .iter()
            .position(|&p| p == pid)
            .unwrap_or(usize::MAX)
    });

    let work_area = resolve_work_area(monitor_id)?;
    let area_x = work_area.left;
    let area_y = work_area.top;
    let area_w = (work_area.right - work_area.left).max(1);
    let area_h = (work_area.bottom - work_area.top).max(1);

    let count = windows.len();
    let (cols, rows) = compute_grid(count);

    let cell_w = area_w / (cols as i32);
    let cell_h = area_h / (rows as i32);

    let mut arranged = 0usize;

    for (idx, &(pid, hwnd)) in windows.iter().enumerate() {
        let col = (idx % cols) as i32;
        let row = (idx / cols) as i32;

        let x = area_x + col * cell_w;
        let y = area_y + row * cell_h;
        // Fill any rounding remainder on the last column/row cleanly
        let w = if col == (cols as i32) - 1 {
            work_area.right - x
        } else {
            cell_w
        };
        let h = if row == (rows as i32) - 1 {
            work_area.bottom - y
        } else {
            cell_h
        };

        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            if let Some(username) = pid_usernames.get(&pid) {
                let title = HSTRING::from(format!("Roblox — @{username}"));
                let _ = SetWindowTextW(hwnd, &title);
            }
            let flags: SET_WINDOW_POS_FLAGS = SWP_NOZORDER | SWP_NOACTIVATE;
            if SetWindowPos(hwnd, None, x, y, w, h, flags).is_ok() {
                arranged += 1;
            }
        }
    }

    Ok(ArrangeWindowsResult {
        arranged_count: arranged,
        cols,
        rows,
    })
}

#[cfg(not(windows))]
pub fn arrange_roblox_windows(
    _tracked_pids: &[u32],
    _all_roblox_pids: &HashSet<u32>,
    _monitor_id: Option<&str>,
    _pid_usernames: &HashMap<u32, String>,
) -> Result<ArrangeWindowsResult, String> {
    Err("Window arrangement is only supported on Windows.".to_string())
}

#[cfg(windows)]
fn resolve_work_area(monitor_id: Option<&str>) -> Result<RECT, String> {
    let monitors = list_monitors();
    if let Some(target_id) = monitor_id.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(m) = monitors.iter().find(|m| m.id == target_id) {
            return Ok(RECT {
                left: m.x,
                top: m.y,
                right: m.x + m.width,
                bottom: m.y + m.height,
            });
        }
    }
    if let Some(primary) = monitors.iter().find(|m| m.is_primary).or(monitors.first()) {
        return Ok(RECT {
            left: primary.x,
            top: primary.y,
            right: primary.x + primary.width,
            bottom: primary.y + primary.height,
        });
    }
    get_desktop_work_area()
}

#[cfg(windows)]
fn get_desktop_work_area() -> Result<RECT, String> {
    let mut rect = RECT::default();
    unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some((&mut rect as *mut RECT).cast::<c_void>()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .map_err(|e| format!("SystemParametersInfoW(SPI_GETWORKAREA) failed: {e}"))?;
    }
    Ok(rect)
}

#[cfg(windows)]
struct EnumContext<'a> {
    target_pids: &'a HashSet<u32>,
    seen_pids: HashSet<u32>,
    found: Vec<(u32, HWND)>,
}

#[cfg(windows)]
unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut EnumContext<'_>);

    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }

    // Skip owned helper / popup / splash windows
    if let Ok(owner) = GetWindow(hwnd, GW_OWNER) {
        if !owner.0.is_null() {
            return BOOL(1);
        }
    }

    // Skip untitled helper windows
    if GetWindowTextLengthW(hwnd) <= 0 {
        return BOOL(1);
    }

    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid != 0 && ctx.target_pids.contains(&pid) && ctx.seen_pids.insert(pid) {
        ctx.found.push((pid, hwnd));
    }

    BOOL(1)
}

#[cfg(windows)]
fn find_top_level_windows_for_pids(target_pids: &HashSet<u32>) -> Vec<(u32, HWND)> {
    let mut ctx = EnumContext {
        target_pids,
        seen_pids: HashSet::new(),
        found: Vec::new(),
    };
    unsafe {
        let _ = EnumWindows(
            Some(enum_windows_proc),
            LPARAM((&mut ctx as *mut EnumContext<'_>) as isize),
        );
    }
    ctx.found
}
