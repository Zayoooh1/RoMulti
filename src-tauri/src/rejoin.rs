use crate::launcher::{
    is_pid_alive, kill_process_by_pid, launch_single_account, snapshot_roblox_pids,
};
use crate::optimizer::PerformanceManager;
use crate::roblox_api::RobloxApiClient;
use crate::storage::AccountStorage;
use crate::window_manager;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::Notify;

const GRACE_PERIOD_SECS: u64 = 30;
const HEALTH_POLL_SECS: u64 = 15;
const MAX_CONSECUTIVE_FAILURES: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Launching,
    InGame,
    Rejoining,
    Error,
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSessionInfo {
    pub user_id: u64,
    pub username: String,
    pub place_id: u64,
    pub job_id: Option<String>,
    pub private_server_link: Option<String>,
    pub pid: Option<u32>,
    pub status: SessionStatus,
    pub consecutive_failures: u32,
    pub last_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoRejoinConfig {
    pub enabled: bool,
    pub interval_minutes: u64,
    pub auto_arrange: bool,
}

impl Default for AutoRejoinConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_minutes: 15,
            auto_arrange: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RejoinTriggeredEvent {
    pub user_id: u64,
    pub username: String,
    pub reason: String,
    pub attempt: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RejoinStatusEvent {
    pub enabled: bool,
    pub interval_minutes: u64,
    pub auto_arrange: bool,
}

#[derive(Debug, Clone)]
struct TrackedSession {
    user_id: u64,
    username: String,
    place_id: u64,
    job_id: Option<String>,
    private_server_link: Option<String>,
    pid: Option<u32>,
    status: SessionStatus,
    launched_at: Instant,
    consecutive_failures: u32,
    last_message: String,
}

impl TrackedSession {
    fn to_info(&self) -> ActiveSessionInfo {
        ActiveSessionInfo {
            user_id: self.user_id,
            username: self.username.clone(),
            place_id: self.place_id,
            job_id: self.job_id.clone(),
            private_server_link: self.private_server_link.clone(),
            pid: self.pid,
            status: self.status,
            consecutive_failures: self.consecutive_failures,
            last_message: self.last_message.clone(),
        }
    }
}

pub struct AutoRejoinManager {
    sessions: Mutex<HashMap<u64, TrackedSession>>,
    config: Mutex<AutoRejoinConfig>,
    running: AtomicBool,
    generation: AtomicU64,
    wakeup: Arc<Notify>,
}

impl AutoRejoinManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            config: Mutex::new(AutoRejoinConfig::default()),
            running: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            wakeup: Arc::new(Notify::new()),
        }
    }

    pub fn get_config(&self) -> AutoRejoinConfig {
        self.config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn get_active_sessions(&self) -> Vec<ActiveSessionInfo> {
        let guard = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let mut list: Vec<ActiveSessionInfo> = guard.values().map(TrackedSession::to_info).collect();
        list.sort_by_key(|s| s.user_id);
        list
    }

    pub fn tracked_pids(&self) -> Vec<u32> {
        let guard = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        guard.values().filter_map(|s| s.pid).collect()
    }

    pub fn tracked_pid_set(&self) -> HashSet<u32> {
        let guard = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        guard.values().filter_map(|s| s.pid).collect()
    }

    pub fn upsert_session(
        &self,
        app: &AppHandle,
        user_id: u64,
        username: String,
        place_id: u64,
        job_id: Option<String>,
        private_server_link: Option<String>,
        pid: Option<u32>,
        status: SessionStatus,
        message: String,
    ) {
        let info = {
            let mut guard = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
            let session = TrackedSession {
                user_id,
                username,
                place_id,
                job_id,
                private_server_link,
                pid,
                status,
                launched_at: Instant::now(),
                consecutive_failures: 0,
                last_message: message,
            };
            let info = session.to_info();
            guard.insert(user_id, session);
            info
        };
        let _ = app.emit("session-updated", info);
    }

    pub fn set_session_status(
        &self,
        app: &AppHandle,
        user_id: u64,
        status: SessionStatus,
        pid: Option<Option<u32>>,
        message: String,
        reset_grace: bool,
        consecutive_failures: Option<u32>,
    ) {
        let maybe_info = {
            let mut guard = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
            guard.get_mut(&user_id).map(|session| {
                session.status = status;
                if let Some(new_pid) = pid {
                    session.pid = new_pid;
                }
                session.last_message = message;
                if reset_grace {
                    session.launched_at = Instant::now();
                }
                if let Some(fails) = consecutive_failures {
                    session.consecutive_failures = fails;
                }
                session.to_info()
            })
        };

        if let Some(info) = maybe_info {
            let _ = app.emit("session-updated", info);
        }
    }

    pub fn remove_session(&self, user_id: u64) {
        let mut guard = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        guard.remove(&user_id);
    }

    pub fn set_config(
        self: &Arc<Self>,
        app: AppHandle,
        storage: Arc<AccountStorage>,
        api: Arc<RobloxApiClient>,
        performance: Arc<PerformanceManager>,
        mut new_config: AutoRejoinConfig,
    ) -> AutoRejoinConfig {
        new_config.interval_minutes = new_config.interval_minutes.clamp(1, 240);

        {
            let mut guard = self.config.lock().unwrap_or_else(|e| e.into_inner());
            *guard = new_config.clone();
        }

        if new_config.enabled {
            // Reset failure counters when user explicitly enables/re-enables Auto Rejoin
            {
                let mut guard = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
                for session in guard.values_mut() {
                    if session.consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                        session.consecutive_failures = 0;
                    }
                }
            }

            if !self.running.swap(true, Ordering::SeqCst) {
                let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
                let manager = Arc::clone(self);
                let app_clone = app.clone();

                tauri::async_runtime::spawn(async move {
                    manager
                        .monitor_loop(gen, app_clone, storage, api, performance)
                        .await;
                });
            } else {
                // Wake existing loop so updated interval_minutes takes effect immediately
                self.wakeup.notify_waiters();
            }
        } else {
            self.stop_internal(&app);
        }

        let _ = app.emit(
            "rejoin-status",
            RejoinStatusEvent {
                enabled: new_config.enabled,
                interval_minutes: new_config.interval_minutes,
                auto_arrange: new_config.auto_arrange,
            },
        );

        new_config
    }

    pub fn stop(&self, app: &AppHandle) -> AutoRejoinConfig {
        self.stop_internal(app);
        let cfg = self.get_config();
        let _ = app.emit(
            "rejoin-status",
            RejoinStatusEvent {
                enabled: cfg.enabled,
                interval_minutes: cfg.interval_minutes,
                auto_arrange: cfg.auto_arrange,
            },
        );
        cfg
    }

    fn stop_internal(&self, _app: &AppHandle) {
        {
            let mut guard = self.config.lock().unwrap_or_else(|e| e.into_inner());
            guard.enabled = false;
        }
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.running.store(false, Ordering::SeqCst);
        self.wakeup.notify_waiters();
    }

    fn is_cancelled(&self, expected_gen: u64) -> bool {
        !self.running.load(Ordering::SeqCst)
            || self.generation.load(Ordering::SeqCst) != expected_gen
    }

    async fn sleep_cancellable(&self, expected_gen: u64, duration: Duration) -> bool {
        if self.is_cancelled(expected_gen) {
            return true;
        }
        tokio::select! {
            _ = tokio::time::sleep(duration) => self.is_cancelled(expected_gen),
            _ = self.wakeup.notified() => self.is_cancelled(expected_gen),
        }
    }

    async fn monitor_loop(
        self: Arc<Self>,
        expected_gen: u64,
        app: AppHandle,
        storage: Arc<AccountStorage>,
        api: Arc<RobloxApiClient>,
        performance: Arc<PerformanceManager>,
    ) {
        while !self.is_cancelled(expected_gen) {
            if self
                .sleep_cancellable(expected_gen, Duration::from_secs(HEALTH_POLL_SECS))
                .await
            {
                break;
            }

            let interval_minutes = self.get_config().interval_minutes.clamp(1, 240);
            let scheduled_rejoin_after = Duration::from_secs(interval_minutes * 60);

            let snapshot: Vec<TrackedSession> = {
                let guard = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
                guard.values().cloned().collect()
            };

            for session in snapshot {
                if self.is_cancelled(expected_gen) {
                    break;
                }

                // Skip sessions that have already hit the max consecutive retry limit or are actively launching
                if session.consecutive_failures >= MAX_CONSECUTIVE_FAILURES
                    || session.status == SessionStatus::Launching
                    || session.status == SessionStatus::Stopped
                {
                    continue;
                }

                // Respect 30-second grace period after launch/rejoin so Roblox can finish loading
                let elapsed = session.launched_at.elapsed();
                if elapsed < Duration::from_secs(GRACE_PERIOD_SECS) {
                    continue;
                }

                let mut disconnect_reason: Option<String> = None;

                // 0. Scheduled periodic rejoin every N minutes (prevents 20-min AFK kick & memory creep)
                if elapsed >= scheduled_rejoin_after {
                    disconnect_reason = Some(format!(
                        "Scheduled Auto Rejoin ({interval_minutes} min elapsed)"
                    ));
                }

                // 1. Check whether tracked PID is still alive
                if disconnect_reason.is_none() {
                    if let Some(pid) = session.pid {
                        if !is_pid_alive(pid) {
                            disconnect_reason =
                                Some(format!("Roblox process (PID {pid}) exited unexpectedly"));
                        }
                    }
                }

                // 2. If process is still alive (or PID wasn't captured), check Roblox Presence API
                if disconnect_reason.is_none() {
                    match storage.get_account_with_cookie(session.user_id) {
                        Ok((_acc, cookie)) => {
                            match api.fetch_user_presence(session.user_id, &cookie).await {
                                Ok(presence) => {
                                    if presence.user_presence_type == 2 {
                                        // Confirmed in-game
                                        if session.status != SessionStatus::InGame
                                            || session.consecutive_failures > 0
                                        {
                                            self.set_session_status(
                                                &app,
                                                session.user_id,
                                                SessionStatus::InGame,
                                                None,
                                                format!("In game (PlaceId {})", session.place_id),
                                                false,
                                                Some(0),
                                            );
                                        }
                                    } else {
                                        disconnect_reason = Some(format!(
                                            "Presence API reported not in game (type={})",
                                            presence.user_presence_type
                                        ));
                                    }
                                }
                                Err(err) => {
                                    // Transient API/network failure shouldn't falsely kill a running game;
                                    // only report expired session if cookie was rejected.
                                    if err.to_lowercase().contains("rejected")
                                        || err.to_lowercase().contains("expired")
                                    {
                                        self.set_session_status(
                                            &app,
                                            session.user_id,
                                            SessionStatus::Error,
                                            None,
                                            err,
                                            false,
                                            Some(MAX_CONSECUTIVE_FAILURES),
                                        );
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            self.set_session_status(
                                &app,
                                session.user_id,
                                SessionStatus::Error,
                                None,
                                err,
                                false,
                                Some(MAX_CONSECUTIVE_FAILURES),
                            );
                            continue;
                        }
                    }
                }

                let Some(reason) = disconnect_reason else {
                    continue;
                };

                if self.is_cancelled(expected_gen) {
                    break;
                }

                let attempt = session.consecutive_failures + 1;

                // Kill any lingering hung Roblox process before rejoining
                if let Some(pid) = session.pid {
                    let _ = kill_process_by_pid(pid);
                }

                self.set_session_status(
                    &app,
                    session.user_id,
                    SessionStatus::Rejoining,
                    Some(None),
                    format!("Rejoining ({attempt}/{MAX_CONSECUTIVE_FAILURES}): {reason}"),
                    false,
                    None,
                );

                let _ = app.emit(
                    "rejoin-triggered",
                    RejoinTriggeredEvent {
                        user_id: session.user_id,
                        username: session.username.clone(),
                        reason: reason.clone(),
                        attempt,
                    },
                );

                // Wait 3 seconds before re-launching (cancellable)
                if self
                    .sleep_cancellable(expected_gen, Duration::from_secs(3))
                    .await
                {
                    break;
                }

                let excluded_pids = self.tracked_pid_set();
                let low_cpu = performance.is_low_cpu_enabled();

                match launch_single_account(
                    &storage,
                    &api,
                    session.user_id,
                    session.place_id,
                    session.job_id.as_deref(),
                    session.private_server_link.as_deref(),
                    &excluded_pids,
                    low_cpu,
                )
                .await
                {
                    Ok(outcome) => {
                        let msg = match outcome.pid {
                            Some(pid) => format!(
                                "Rejoined {} (PID {pid}) into Place {}",
                                outcome.username, session.place_id
                            ),
                            None => format!(
                                "Rejoined {} into Place {}",
                                outcome.username, session.place_id
                            ),
                        };
                        self.set_session_status(
                            &app,
                            session.user_id,
                            SessionStatus::InGame,
                            Some(outcome.pid),
                            msg,
                            true, // Reset 30s grace period
                            Some(0),
                        );

                        // If auto_arrange is enabled, arrange windows after rejoin window opens
                        if self.get_config().auto_arrange {
                            if !self
                                .sleep_cancellable(expected_gen, Duration::from_secs(4))
                                .await
                            {
                                let tracked = self.tracked_pids();
                                let all_pids = snapshot_roblox_pids();
                                let _ =
                                    window_manager::arrange_roblox_windows(&tracked, &all_pids);
                            }
                        }
                    }
                    Err(err) => {
                        let next_fails = session.consecutive_failures + 1;
                        let exhausted = next_fails >= MAX_CONSECUTIVE_FAILURES;
                        let msg = if exhausted {
                            format!(
                                "Auto Rejoin paused after {MAX_CONSECUTIVE_FAILURES} failed attempts: {err}"
                            )
                        } else {
                            format!(
                                "Rejoin attempt {next_fails}/{MAX_CONSECUTIVE_FAILURES} failed: {err}"
                            )
                        };

                        self.set_session_status(
                            &app,
                            session.user_id,
                            SessionStatus::Error,
                            Some(None),
                            msg,
                            false,
                            Some(next_fails),
                        );
                    }
                }
            }
        }

        // Clear running flag only if we are still the active generation
        if self.generation.load(Ordering::SeqCst) == expected_gen {
            self.running.store(false, Ordering::SeqCst);
        }
    }
}
