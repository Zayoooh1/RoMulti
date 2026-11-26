mod audio;
mod auth_window;
mod config;
mod launcher;
mod mutex;
mod notifications;
mod optimizer;
mod rejoin;
mod roblox_api;
mod storage;
mod window_manager;

use audio::AudioApplyResult;
use config::{AppConfig, AppSettings, ConfigStorage, FavoriteGame, LaunchProfile, MuteMode};
use launcher::{
    kill_process_by_pid, launch_single_account, snapshot_roblox_pids, spawn_delayed_mute_apply,
    LaunchManager,
};
use mutex::{MutexManager, MutexStatus};
use notifications::NotificationManager;
use optimizer::{ApplyPerformanceResult, PerformanceManager, PerformanceSettings};
use rejoin::{ActiveSessionInfo, AutoRejoinConfig, AutoRejoinManager, SessionStatus};
use roblox_api::{GameSearchResult, PublicServersPage, RobloxApiClient};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use storage::{AccountStorage, PublicAccount};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WindowEvent,
};
use window_manager::{ArrangeWindowsResult, MonitorInfo};

struct AppState {
    mutex_manager: MutexManager,
    storage: Arc<AccountStorage>,
    config: Arc<ConfigStorage>,
    api: Arc<RobloxApiClient>,
    launcher: Arc<LaunchManager>,
    rejoin: Arc<AutoRejoinManager>,
    performance: Arc<PerformanceManager>,
    notifications: Arc<NotificationManager>,
}

#[tauri::command]
fn get_mutex_status(state: State<'_, AppState>) -> MutexStatus {
    state.mutex_manager.status()
}

#[tauri::command]
fn reinitialize_mutex(state: State<'_, AppState>) -> MutexStatus {
    state.mutex_manager.reinitialize()
}

#[tauri::command]
fn get_accounts(state: State<'_, AppState>) -> Result<Vec<PublicAccount>, String> {
    state.storage.list_public_accounts()
}

#[tauri::command]
fn remove_account(state: State<'_, AppState>, user_id: u64) -> Result<(), String> {
    state.rejoin.remove_session(user_id);
    state.storage.remove_account(user_id)
}

#[tauri::command]
fn update_account_tags(
    state: State<'_, AppState>,
    user_id: u64,
    tags: Vec<String>,
) -> Result<PublicAccount, String> {
    state.storage.update_account_tags(user_id, tags)
}

#[tauri::command]
async fn verify_account(
    state: State<'_, AppState>,
    user_id: u64,
) -> Result<PublicAccount, String> {
    let (_acc, cookie) = state.storage.get_account_with_cookie(user_id)?;
    let user = state.api.fetch_authenticated_user(&cookie).await?;
    let avatar_url = state
        .api
        .fetch_user_avatar_headshot(user.id)
        .await
        .unwrap_or_default();

    state
        .storage
        .update_account_profile(user.id, user.name, user.display_name, avatar_url)
}

#[tauri::command]
async fn open_auth_window(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    auth_window::open_roblox_auth_window(
        app,
        Arc::clone(&state.storage),
        Arc::clone(&state.api),
    )
    .await
}

#[tauri::command]
async fn search_games(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<GameSearchResult>, String> {
    state.api.search_games(&query).await
}

#[tauri::command]
async fn get_public_servers(
    state: State<'_, AppState>,
    place_id: u64,
    sort_order: String,
    cursor: Option<String>,
) -> Result<PublicServersPage, String> {
    state
        .api
        .fetch_public_servers(place_id, &sort_order, cursor.as_deref(), 50)
        .await
}

#[tauri::command]
fn get_app_config(state: State<'_, AppState>) -> AppConfig {
    state.config.get_config()
}

#[tauri::command]
fn save_favorite_game(
    state: State<'_, AppState>,
    favorite: FavoriteGame,
) -> Result<AppConfig, String> {
    state.config.save_favorite_game(favorite)
}

#[tauri::command]
fn remove_favorite_game(state: State<'_, AppState>, place_id: u64) -> Result<AppConfig, String> {
    state.config.remove_favorite_game(place_id)
}

#[tauri::command]
fn save_launch_profile(
    state: State<'_, AppState>,
    profile: LaunchProfile,
) -> Result<AppConfig, String> {
    state.config.save_launch_profile(profile)
}

#[tauri::command]
fn delete_launch_profile(state: State<'_, AppState>, id: String) -> Result<AppConfig, String> {
    state.config.delete_launch_profile(&id)
}

#[tauri::command]
fn update_app_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppConfig, String> {
    state.config.update_app_settings(settings)
}

#[tauri::command]
async fn test_discord_webhook(state: State<'_, AppState>, url: String) -> Result<(), String> {
    state.notifications.send_test_webhook(&url).await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn launch_accounts(
    app: AppHandle,
    state: State<'_, AppState>,
    user_ids: Vec<u64>,
    place_id: u64,
    job_id: Option<String>,
    private_server_link: Option<String>,
    delay_seconds: u64,
    auto_arrange: Option<bool>,
    join_same_server: Option<bool>,
    monitor_id: Option<String>,
    mute_mode: Option<MuteMode>,
) -> Result<(), String> {
    // Automatically close conflicting Roblox instances & re-acquire mutex if needed
    if !state.mutex_manager.is_active() {
        let updated_status = state.mutex_manager.ensure_ready_for_launch();
        let _ = app.emit("mutex-status", updated_status);
    }

    let should_auto_arrange =
        auto_arrange.unwrap_or_else(|| state.rejoin.get_config().auto_arrange);
    state.launcher.start_launch(
        app,
        Arc::clone(&state.storage),
        Arc::clone(&state.api),
        Arc::clone(&state.rejoin),
        Arc::clone(&state.performance),
        Arc::clone(&state.config),
        Arc::clone(&state.notifications),
        user_ids,
        place_id,
        job_id,
        private_server_link,
        delay_seconds,
        should_auto_arrange,
        join_same_server.unwrap_or(false),
        monitor_id,
        mute_mode,
    )
}

#[tauri::command]
fn cancel_launch(state: State<'_, AppState>) {
    state.launcher.request_cancel();
}

#[tauri::command]
fn get_auto_rejoin_config(state: State<'_, AppState>) -> AutoRejoinConfig {
    state.rejoin.get_config()
}

#[tauri::command]
fn set_auto_rejoin_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: AutoRejoinConfig,
) -> AutoRejoinConfig {
    state.rejoin.set_config(
        app,
        Arc::clone(&state.storage),
        Arc::clone(&state.api),
        Arc::clone(&state.performance),
        Arc::clone(&state.config),
        Arc::clone(&state.notifications),
        config,
    )
}

#[tauri::command]
fn stop_auto_rejoin(app: AppHandle, state: State<'_, AppState>) -> AutoRejoinConfig {
    state.rejoin.stop(&app)
}

#[tauri::command]
fn get_active_sessions(state: State<'_, AppState>) -> Vec<ActiveSessionInfo> {
    state.rejoin.get_active_sessions()
}

#[tauri::command]
fn focus_account_window(state: State<'_, AppState>, user_id: u64) -> Result<(), String> {
    let session = state
        .rejoin
        .get_session(user_id)
        .ok_or_else(|| "No active session found for this account.".to_string())?;
    let pid = session
        .pid
        .ok_or_else(|| "No process PID recorded for this account.".to_string())?;
    window_manager::focus_window_for_pid(pid)
}

#[tauri::command]
fn kill_account_session(
    app: AppHandle,
    state: State<'_, AppState>,
    user_id: u64,
) -> Result<(), String> {
    let session = state
        .rejoin
        .get_session(user_id)
        .ok_or_else(|| "No active session found for this account.".to_string())?;

    if let Some(pid) = session.pid {
        kill_process_by_pid(pid)?;
    }

    state.rejoin.set_session_status(
        &app,
        user_id,
        SessionStatus::Stopped,
        Some(None),
        "Stopped manually by user.".to_string(),
        false,
        Some(0),
    );

    Ok(())
}

#[tauri::command]
async fn rejoin_single_account(
    app: AppHandle,
    state: State<'_, AppState>,
    user_id: u64,
) -> Result<ActiveSessionInfo, String> {
    let session = state
        .rejoin
        .get_session(user_id)
        .ok_or_else(|| "No previous session found to rejoin for this account.".to_string())?;

    if !state.mutex_manager.is_active() {
        let updated_status = state.mutex_manager.ensure_ready_for_launch();
        let _ = app.emit("mutex-status", updated_status);
    }

    if let Some(pid) = session.pid {
        let _ = kill_process_by_pid(pid);
    }

    state.rejoin.set_session_status(
        &app,
        user_id,
        SessionStatus::Rejoining,
        Some(None),
        format!("Manually rejoining @{}...", session.username),
        false,
        Some(0),
    );

    tokio::time::sleep(Duration::from_secs(2)).await;

    let excluded_pids = state.rejoin.tracked_pid_set();
    let low_cpu = state.performance.is_low_cpu_enabled();

    match launch_single_account(
        &state.storage,
        &state.api,
        user_id,
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
                    "Rejoined @{} [PID {pid}] into Place {}",
                    outcome.username, session.place_id
                ),
                None => format!(
                    "Rejoined @{} into Place {}",
                    outcome.username, session.place_id
                ),
            };
            state.rejoin.set_session_status(
                &app,
                user_id,
                SessionStatus::InGame,
                Some(outcome.pid),
                msg,
                true,
                Some(0),
            );
            if let Some(pid) = outcome.pid {
                window_manager::spawn_window_title_setter(pid, outcome.username);
            }
            spawn_delayed_mute_apply(
                Arc::clone(&state.rejoin),
                Arc::clone(&state.config),
                None,
            );
            state
                .rejoin
                .get_session(user_id)
                .ok_or_else(|| "Session state missing after rejoin.".to_string())
        }
        Err(err) => {
            let lower = err.to_lowercase();
            if lower.contains("rejected") || lower.contains("expired") {
                state.notifications.notify_expired_cookie(
                    &state.config.get_app_settings(),
                    &session.username,
                    &err,
                );
            }
            state.rejoin.set_session_status(
                &app,
                user_id,
                SessionStatus::Error,
                Some(None),
                err.clone(),
                false,
                None,
            );
            Err(err)
        }
    }
}

#[tauri::command]
fn list_monitors() -> Vec<MonitorInfo> {
    window_manager::list_monitors()
}

#[tauri::command]
fn arrange_windows(
    state: State<'_, AppState>,
    monitor_id: Option<String>,
) -> Result<ArrangeWindowsResult, String> {
    let tracked = state.rejoin.tracked_pids();
    let all_roblox = snapshot_roblox_pids();
    let pid_usernames = state.rejoin.tracked_pid_usernames();
    let effective_monitor = monitor_id
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| state.config.get_app_settings().default_monitor_id);
    window_manager::arrange_roblox_windows(
        &tracked,
        &all_roblox,
        effective_monitor.as_deref(),
        &pid_usernames,
    )
}

#[tauri::command]
async fn apply_audio_mute_mode(
    state: State<'_, AppState>,
    mode: MuteMode,
) -> Result<AudioApplyResult, String> {
    let _ = state.config.set_mute_mode(mode);
    let tracked = state.rejoin.tracked_pids();
    let all_roblox = snapshot_roblox_pids();
    tokio::task::spawn_blocking(move || audio::apply_mute_mode(mode, &tracked, &all_roblox))
        .await
        .map_err(|e| format!("Audio mute task failed: {e}"))?
}

#[tauri::command]
async fn toggle_account_mute(
    state: State<'_, AppState>,
    user_id: u64,
    muted: bool,
) -> Result<bool, String> {
    let session = state
        .rejoin
        .get_session(user_id)
        .ok_or_else(|| "No active session found for this account.".to_string())?;
    let pid = session
        .pid
        .ok_or_else(|| "No active process PID for this account.".to_string())?;
    tokio::task::spawn_blocking(move || audio::set_pid_muted(pid, muted))
        .await
        .map_err(|e| format!("Audio mute task failed: {e}"))?
}

#[tauri::command]
fn get_performance_settings(state: State<'_, AppState>) -> PerformanceSettings {
    state.performance.get_settings()
}

#[tauri::command]
fn apply_performance_settings(
    state: State<'_, AppState>,
    settings: PerformanceSettings,
) -> Result<ApplyPerformanceResult, String> {
    let mut target_pids: HashSet<u32> = state.rejoin.tracked_pid_set();
    target_pids.extend(snapshot_roblox_pids());
    state.performance.apply(settings, &target_pids)
}

fn show_and_focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Acquire ROBLOX_singletonEvent and ROBLOX_singletonMutex immediately on startup
    // and hold both handles for the entire lifecycle of RoMulti.
    let mutex_manager = MutexManager::new();
    let storage = Arc::new(
        AccountStorage::new().expect("Failed to initialize RoMulti account storage"),
    );
    let config = Arc::new(
        ConfigStorage::new().expect("Failed to initialize RoMulti config storage"),
    );
    let api = Arc::new(
        RobloxApiClient::new().expect("Failed to initialize Roblox API client"),
    );
    let launcher = Arc::new(LaunchManager::new());
    let rejoin = Arc::new(AutoRejoinManager::new());
    let performance = Arc::new(PerformanceManager::new());
    let notifications = Arc::new(
        NotificationManager::new().expect("Failed to initialize notification manager"),
    );

    let app_state = AppState {
        mutex_manager,
        storage,
        config,
        api,
        launcher,
        rejoin,
        performance,
        notifications,
    };

    tauri::Builder::default()
        .manage(app_state)
        .setup(|app| {
            let show_hide = MenuItem::with_id(app, "tray_show_hide", "Show / Hide RoMulti", true, None::<&str>)?;
            let arrange = MenuItem::with_id(app, "tray_arrange", "Arrange Windows", true, None::<&str>)?;
            let mute_all = MenuItem::with_id(app, "tray_mute_all", "Mute All Roblox", true, None::<&str>)?;
            let stop_rejoin = MenuItem::with_id(app, "tray_stop_rejoin", "Stop Auto Rejoin", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "tray_quit", "Quit RoMulti", true, None::<&str>)?;

            let menu = Menu::with_items(
                app,
                &[&show_hide, &arrange, &mute_all, &stop_rejoin, &sep, &quit],
            )?;

            let mut tray_builder = TrayIconBuilder::with_id("romulti-tray")
                .tooltip("RoMulti — Roblox Multi-Instance Launcher")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app_handle, event| match event.id.as_ref() {
                    "tray_show_hide" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                show_and_focus_main_window(app_handle);
                            }
                        }
                    }
                    "tray_arrange" => {
                        let state = app_handle.state::<AppState>();
                        let tracked = state.rejoin.tracked_pids();
                        let all_roblox = snapshot_roblox_pids();
                        let pid_usernames = state.rejoin.tracked_pid_usernames();
                        let monitor_id = state.config.get_app_settings().default_monitor_id;
                        let _ = window_manager::arrange_roblox_windows(
                            &tracked,
                            &all_roblox,
                            monitor_id.as_deref(),
                            &pid_usernames,
                        );
                    }
                    "tray_mute_all" => {
                        let state = app_handle.state::<AppState>();
                        let _ = state.config.set_mute_mode(MuteMode::MuteAll);
                        let tracked = state.rejoin.tracked_pids();
                        let all_roblox = snapshot_roblox_pids();
                        let app_clone = app_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = tokio::task::spawn_blocking(move || {
                                audio::apply_mute_mode(MuteMode::MuteAll, &tracked, &all_roblox)
                            })
                            .await;
                            let _ = app_clone.emit("mute-mode-changed", MuteMode::MuteAll);
                        });
                    }
                    "tray_stop_rejoin" => {
                        let state = app_handle.state::<AppState>();
                        let _ = state.rejoin.stop(app_handle);
                    }
                    "tray_quit" => {
                        app_handle.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_and_focus_main_window(tray.app_handle());
                    }
                });

            if let Some(icon) = app.default_window_icon().cloned() {
                tray_builder = tray_builder.icon(icon);
            }

            tray_builder.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    let state = window.state::<AppState>();
                    if state.config.get_app_settings().minimize_to_tray {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_mutex_status,
            reinitialize_mutex,
            get_accounts,
            remove_account,
            update_account_tags,
            verify_account,
            open_auth_window,
            search_games,
            get_public_servers,
            get_app_config,
            save_favorite_game,
            remove_favorite_game,
            save_launch_profile,
            delete_launch_profile,
            update_app_settings,
            test_discord_webhook,
            launch_accounts,
            cancel_launch,
            get_auto_rejoin_config,
            set_auto_rejoin_config,
            stop_auto_rejoin,
            get_active_sessions,
            focus_account_window,
            kill_account_session,
            rejoin_single_account,
            list_monitors,
            arrange_windows,
            apply_audio_mute_mode,
            toggle_account_mute,
            get_performance_settings,
            apply_performance_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running RoMulti application");
}
