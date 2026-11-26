use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use windows::{
    core::BOOL,
    Win32::{
        Foundation::{HWND, LPARAM, RECT},
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindow, GetWindowTextLengthW, GetWindowThreadProcessId, IsIconic,
            IsWindowVisible, SetWindowPos, ShowWindow, SystemParametersInfoW, GW_OWNER,
            SET_WINDOW_POS_FLAGS, SPI_GETWORKAREA, SWP_NOACTIVATE, SWP_NOZORDER, SW_RESTORE,
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

/// Arranges visible top-level Roblox windows in an even grid across the desktop work area
/// (excluding the Windows taskbar).
/// If `tracked_pids` is non-empty and matches visible windows, arranges those; otherwise
/// falls back to all currently running `RobloxPlayerBeta.exe` processes.
#[cfg(windows)]
pub fn arrange_roblox_windows(
    tracked_pids: &[u32],
    all_roblox_pids: &HashSet<u32>,
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

    let work_area = get_desktop_work_area()?;
    let area_x = work_area.left;
    let area_y = work_area.top;
    let area_w = (work_area.right - work_area.left).max(1);
    let area_h = (work_area.bottom - work_area.top).max(1);

    let count = windows.len();
    let (cols, rows) = compute_grid(count);

    let cell_w = area_w / (cols as i32);
    let cell_h = area_h / (rows as i32);

    let mut arranged = 0usize;

    for (idx, &(_pid, hwnd)) in windows.iter().enumerate() {
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
) -> Result<ArrangeWindowsResult, String> {
    Err("Window arrangement is only supported on Windows.".to_string())
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
