use crate::optimizer::{apply_priority_to_pid, PerformanceManager};
use crate::rejoin::{AutoRejoinManager, SessionStatus};
use crate::roblox_api::RobloxApiClient;
use crate::storage::AccountStorage;
use crate::window_manager;
use rand::Rng;
use serde::Serialize;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

#[cfg(windows)]
use windows::{
    core::{w, HSTRING},
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            Threading::{
                GetExitCodeProcess, OpenProcess, TerminateProcess,
                PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
            },
        },
        UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    },
};

const PID_CAPTURE_TIMEOUT_SECS: u64 = 10;
const STILL_ACTIVE_EXIT_CODE: u32 = 259;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedPrivateServerLink {
    Classic { link_code: String },
    Share { code: String },
}

#[derive(Debug, Clone)]
pub struct SingleLaunchOutcome {
    pub username: String,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchProgressEvent {
    pub index: usize,
    pub total: usize,
    pub user_id: u64,
    pub username: String,
    pub status: String, // "launching" | "launched" | "waiting" | "error"
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchCompleteEvent {
    pub launched: usize,
    pub failed: usize,
    pub cancelled: bool,
}

pub struct LaunchManager {
    is_launching: AtomicBool,
    cancel_requested: AtomicBool,
}

impl LaunchManager {
    pub fn new() -> Self {
        Self {
            is_launching: AtomicBool::new(false),
            cancel_requested: AtomicBool::new(false),
        }
    }

    pub fn request_cancel(&self) {
        if self.is_launching.load(Ordering::SeqCst) {
            self.cancel_requested.store(true, Ordering::SeqCst);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_launch(
        self: &Arc<Self>,
        app: AppHandle,
        storage: Arc<AccountStorage>,
        api: Arc<RobloxApiClient>,
        rejoin_manager: Arc<AutoRejoinManager>,
        performance: Arc<PerformanceManager>,
        user_ids: Vec<u64>,
        place_id: u64,
        job_id: Option<String>,
        private_server_link: Option<String>,
        delay_seconds: u64,
        auto_arrange: bool,
    ) -> Result<(), String> {
        if user_ids.is_empty() {
            return Err("Select at least one account to launch.".to_string());
        }
        if place_id == 0 {
            return Err("Select a valid Roblox experience first.".to_string());
        }

        let cleaned_vip = private_server_link
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        // Validate Private Server link upfront before starting the launch sequence
        if let Some(ref vip) = cleaned_vip {
            parse_private_server_link(vip)?;
        }

        if self
            .is_launching
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("A launch sequence is already in progress.".to_string());
        }
        self.cancel_requested.store(false, Ordering::SeqCst);

        let manager = Arc::clone(self);
        let cleaned_job_id = job_id
            .map(|j| j.trim().to_string())
            .filter(|j| !j.is_empty());
        let delay = delay_seconds.clamp(1, 60);

        tauri::async_runtime::spawn(async move {
            let total = user_ids.len();
            let mut launched_count = 0usize;
            let mut failed_count = 0usize;
            let mut cancelled = false;

            for (idx, user_id) in user_ids.into_iter().enumerate() {
                if manager.cancel_requested.load(Ordering::SeqCst) {
                    cancelled = true;
                    break;
                }

                let position = idx + 1;

                // Look up display name for immediate progress feedback
                let preview_name = storage
                    .get_account_with_cookie(user_id)
                    .map(|(acc, _)| acc.username)
                    .unwrap_or_else(|_| format!("User #{user_id}"));

                rejoin_manager.upsert_session(
                    &app,
                    user_id,
                    preview_name.clone(),
                    place_id,
                    cleaned_job_id.clone(),
                    cleaned_vip.clone(),
                    None,
                    SessionStatus::Launching,
                    format!("Launching {preview_name} ({position}/{total})..."),
                );

                let _ = app.emit(
                    "launch-progress",
                    LaunchProgressEvent {
                        index: position,
                        total,
                        user_id,
                        username: preview_name.clone(),
                        status: "launching".to_string(),
                        message: format!(
                            "Generating authentication ticket for {preview_name} ({position}/{total})..."
                        ),
                    },
                );

                let excluded_pids = rejoin_manager.tracked_pid_set();
                let low_cpu = performance.is_low_cpu_enabled();

                match launch_single_account(
                    &storage,
                    &api,
                    user_id,
                    place_id,
                    cleaned_job_id.as_deref(),
                    cleaned_vip.as_deref(),
                    &excluded_pids,
                    low_cpu,
                )
                .await
                {
                    Ok(outcome) => {
                        launched_count += 1;
                        let pid_note = outcome
                            .pid
                            .map(|p| format!(" [PID {p}]"))
                            .unwrap_or_default();
                        let vip_note = if cleaned_vip.is_some() { " (VIP)" } else { "" };
                        let msg = format!(
                            "Launched {}{pid_note} into Place {place_id}{vip_note} ({position}/{total}).",
                            outcome.username
                        );

                        rejoin_manager.upsert_session(
                            &app,
                            user_id,
                            outcome.username.clone(),
                            place_id,
                            cleaned_job_id.clone(),
                            cleaned_vip.clone(),
                            outcome.pid,
                            SessionStatus::InGame,
                            msg.clone(),
                        );

                        let _ = app.emit(
                            "launch-progress",
                            LaunchProgressEvent {
                                index: position,
                                total,
                                user_id,
                                username: outcome.username,
                                status: "launched".to_string(),
                                message: msg,
                            },
                        );
                    }
                    Err(err) => {
                        failed_count += 1;
                        rejoin_manager.upsert_session(
                            &app,
                            user_id,
                            preview_name.clone(),
                            place_id,
                            cleaned_job_id.clone(),
                            cleaned_vip.clone(),
                            None,
                            SessionStatus::Error,
                            err.clone(),
                        );

                        let _ = app.emit(
                            "launch-progress",
                            LaunchProgressEvent {
                                index: position,
                                total,
                                user_id,
                                username: preview_name,
                                status: "error".to_string(),
                                message: err,
                            },
                        );
                    }
                }

                // Wait configurable delay before launching the next instance
                if position < total {
                    let _ = app.emit(
                        "launch-progress",
                        LaunchProgressEvent {
                            index: position,
                            total,
                            user_id,
                            username: String::new(),
                            status: "waiting".to_string(),
                            message: format!("Waiting {delay}s before launching next account..."),
                        },
                    );

                    let steps = delay * 4;
                    for _ in 0..steps {
                        if manager.cancel_requested.load(Ordering::SeqCst) {
                            cancelled = true;
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(250)).await;
                    }

                    if cancelled {
                        break;
                    }
                }
            }

            // Auto-arrange windows after batch launch finishes if enabled
            if auto_arrange && launched_count > 0 && !cancelled {
                tokio::time::sleep(Duration::from_secs(3)).await;
                let tracked = rejoin_manager.tracked_pids();
                let all_pids = snapshot_roblox_pids();
                let _ = window_manager::arrange_roblox_windows(&tracked, &all_pids);
            }

            manager.is_launching.store(false, Ordering::SeqCst);
            manager.cancel_requested.store(false, Ordering::SeqCst);

            let _ = app.emit(
                "launch-complete",
                LaunchCompleteEvent {
                    launched: launched_count,
                    failed: failed_count,
                    cancelled,
                },
            );
        });

        Ok(())
    }
}

/// Launches a single Roblox account, captures its newly spawned `RobloxPlayerBeta.exe` PID
/// (up to 10s timeout), and applies Low CPU priority if enabled.
#[allow(clippy::too_many_arguments)]
pub async fn launch_single_account(
    storage: &Arc<AccountStorage>,
    api: &Arc<RobloxApiClient>,
    user_id: u64,
    place_id: u64,
    job_id: Option<&str>,
    private_server_link: Option<&str>,
    already_tracked_pids: &HashSet<u32>,
    low_cpu_mode: bool,
) -> Result<SingleLaunchOutcome, String> {
    let (account, cookie) = storage.get_account_with_cookie(user_id)?;

    // Resolve Private Server link code if a VIP link was provided
    let resolved_vip_code = match private_server_link {
        Some(raw) if !raw.trim().is_empty() => match parse_private_server_link(raw)? {
            ParsedPrivateServerLink::Classic { link_code } => Some(link_code),
            ParsedPrivateServerLink::Share { code } => {
                Some(api.resolve_share_server_code(&code, &cookie).await)
            }
        },
        _ => None,
    };

    // Request fresh rbx-authentication-ticket
    let ticket = api.generate_auth_ticket(&cookie).await?;

    // Build launch URI
    let uri = build_launch_uri(
        &ticket,
        place_id,
        job_id,
        resolved_vip_code.as_deref(),
    );

    // Snapshot running RobloxPlayerBeta.exe PIDs right before launching
    let mut pids_before = snapshot_roblox_pids();
    pids_before.extend(already_tracked_pids.iter().copied());

    // Open protocol URI via Windows ShellExecuteW
    open_protocol_uri(&uri)?;

    // Wait up to 10s for the newly spawned RobloxPlayerBeta.exe process PID
    let captured_pid = wait_for_new_roblox_pid(&pids_before, Duration::from_secs(PID_CAPTURE_TIMEOUT_SECS)).await;

    // Immediately apply BELOW_NORMAL_PRIORITY_CLASS if Low CPU Mode is active
    if let Some(pid) = captured_pid {
        if low_cpu_mode {
            let _ = apply_priority_to_pid(pid, true);
        }
    }

    Ok(SingleLaunchOutcome {
        username: account.username,
        pid: captured_pid,
    })
}

/// Parses and validates both classic (`?privateServerLinkCode=XXXX`) and modern share
/// (`https://www.roblox.com/share?code=XXXX&type=Server`) Roblox Private Server links.
pub fn parse_private_server_link(raw: &str) -> Result<ParsedPrivateServerLink, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Private server link is empty.".to_string());
    }

    let parsed = url::Url::parse(trimmed).map_err(|_| {
        "Invalid Private Server link URL. Expected a roblox.com link with privateServerLinkCode=... or /share?code=...&type=Server.".to_string()
    })?;

    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    if !host.ends_with("roblox.com") {
        return Err("Private Server link must be a valid roblox.com URL.".to_string());
    }

    let mut private_server_code: Option<String> = None;
    let mut share_code: Option<String> = None;
    let mut share_type: Option<String> = None;

    for (key, val) in parsed.query_pairs() {
        let v = val.trim();
        if v.is_empty() {
            continue;
        }
        if key.eq_ignore_ascii_case("privateServerLinkCode") {
            private_server_code = Some(v.to_string());
        } else if key.eq_ignore_ascii_case("code") {
            share_code = Some(v.to_string());
        } else if key.eq_ignore_ascii_case("type") {
            share_type = Some(v.to_string());
        }
    }

    if let Some(link_code) = private_server_code {
        return Ok(ParsedPrivateServerLink::Classic { link_code });
    }

    if parsed.path().to_ascii_lowercase().starts_with("/share") {
        if let Some(code) = share_code {
            if let Some(ref st) = share_type {
                if !st.eq_ignore_ascii_case("Server") {
                    return Err(format!(
                        "Unsupported share link type '{st}'. Expected 'type=Server' for a Private Server link."
                    ));
                }
            }
            return Ok(ParsedPrivateServerLink::Share { code });
        }
    }

    Err("Unrecognized Private Server link format. Use '...privateServerLinkCode=XXXX' or 'https://www.roblox.com/share?code=XXXX&type=Server'.".to_string())
}

fn build_launch_uri(
    ticket: &str,
    place_id: u64,
    job_id: Option<&str>,
    private_server_code: Option<&str>,
) -> String {
    let mut rng = rand::thread_rng();
    let tracker_id: u64 = rng.gen_range(1_000_000_000..=9_999_999_999);

    let launch_time_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    let place_launcher_url = if let Some(link_code) = private_server_code {
        format!(
            "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestPrivateGame&browserTrackerId={tracker_id}&placeId={place_id}&linkCode={}&isPlayTogetherGame=false",
            urlencoding::encode(link_code)
        )
    } else if let Some(jid) = job_id {
        format!(
            "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestGameJob&browserTrackerId={tracker_id}&placeId={place_id}&gameId={}&isPlayTogetherGame=false",
            urlencoding::encode(jid)
        )
    } else {
        format!(
            "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestGame&browserTrackerId={tracker_id}&placeId={place_id}&isPlayTogetherGame=false"
        )
    };

    let encoded_launcher_url = urlencoding::encode(&place_launcher_url);

    format!(
        "roblox-player:1+launchmode:play+gameinfo:{ticket}+launchtime:{launch_time_ms}+placelauncherurl:{encoded_launcher_url}+browsertrackerid:{tracker_id}+robloxLocale:en_us+gameLocale:en_us+channel:+LaunchExp:InApp"
    )
}

async fn wait_for_new_roblox_pid(before: &HashSet<u32>, timeout: Duration) -> Option<u32> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let current = snapshot_roblox_pids();
        if let Some(&new_pid) = current.difference(before).next() {
            return Some(new_pid);
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    None
}

/// Returns the set of all running `RobloxPlayerBeta.exe` process IDs via Win32 Toolhelp32.
#[cfg(windows)]
pub fn snapshot_roblox_pids() -> HashSet<u32> {
    let mut pids = HashSet::new();
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return pids;
        };
        if snapshot.is_invalid() {
            return pids;
        }

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
                if exe_name.eq_ignore_ascii_case("RobloxPlayerBeta.exe") {
                    pids.insert(entry.th32ProcessID);
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
    }
    pids
}

#[cfg(not(windows))]
pub fn snapshot_roblox_pids() -> HashSet<u32> {
    HashSet::new()
}

/// Checks if a process with the given PID is currently still running.
#[cfg(windows)]
pub fn is_pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        if handle.is_invalid() {
            return false;
        }
        let mut exit_code: u32 = 0;
        let alive = GetExitCodeProcess(handle, &mut exit_code).is_ok()
            && exit_code == STILL_ACTIVE_EXIT_CODE;
        let _ = CloseHandle(handle);
        alive
    }
}

#[cfg(not(windows))]
pub fn is_pid_alive(_pid: u32) -> bool {
    false
}

/// Terminates a Windows process by PID using Win32 `OpenProcess` + `TerminateProcess`.
#[cfg(windows)]
pub fn kill_process_by_pid(pid: u32) -> Result<(), String> {
    if pid == 0 {
        return Ok(());
    }
    unsafe {
        let handle: HANDLE = match OpenProcess(PROCESS_TERMINATE, false, pid) {
            Ok(h) if !h.is_invalid() => h,
            // Process may already have exited
            _ => return Ok(()),
        };
        let res = TerminateProcess(handle, 1)
            .map_err(|e| format!("TerminateProcess failed for PID {pid}: {e}"));
        let _ = CloseHandle(handle);
        res
    }
}

#[cfg(not(windows))]
pub fn kill_process_by_pid(_pid: u32) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
fn open_protocol_uri(uri: &str) -> Result<(), String> {
    let uri_hstring = HSTRING::from(uri);
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &uri_hstring,
            None,
            None,
            SW_SHOWNORMAL,
        )
    };

    let code = result.0 as isize;
    if code <= 32 {
        return Err(format!(
            "Windows failed to open roblox-player: URI (ShellExecuteW code {code}). Make sure Roblox Player is installed."
        ));
    }

    Ok(())
}

#[cfg(not(windows))]
fn open_protocol_uri(_uri: &str) -> Result<(), String> {
    Err("Launching Roblox instances via roblox-player: is only supported on Windows.".to_string())
}
