mod auth_window;
mod launcher;
mod mutex;
mod optimizer;
mod rejoin;
mod roblox_api;
mod storage;
mod window_manager;

use launcher::{snapshot_roblox_pids, LaunchManager};
use mutex::{MutexManager, MutexStatus};
use optimizer::{ApplyPerformanceResult, PerformanceManager, PerformanceSettings};
use rejoin::{ActiveSessionInfo, AutoRejoinConfig, AutoRejoinManager};
use roblox_api::{GameSearchResult, RobloxApiClient};
use std::collections::HashSet;
use std::sync::Arc;
use storage::{AccountStorage, PublicAccount};
use tauri::{AppHandle, Emitter, State};
use window_manager::ArrangeWindowsResult;

struct AppState {
    mutex_manager: MutexManager,
    storage: Arc<AccountStorage>,
    api: Arc<RobloxApiClient>,
    launcher: Arc<LaunchManager>,
    rejoin: Arc<AutoRejoinManager>,
    performance: Arc<PerformanceManager>,
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
) -> Result<(), String> {
    // Automatically close conflicting Roblox instances & re-acquire mutex if needed
    if !state.mutex_manager.is_active() {
        let updated_status = state.mutex_manager.ensure_ready_for_launch();
        let _ = app.emit("mutex-status", updated_status);
    }

    let should_auto_arrange = auto_arrange.unwrap_or_else(|| state.rejoin.get_config().auto_arrange);
    state.launcher.start_launch(
        app,
        Arc::clone(&state.storage),
        Arc::clone(&state.api),
        Arc::clone(&state.rejoin),
        Arc::clone(&state.performance),
        user_ids,
        place_id,
        job_id,
        private_server_link,
        delay_seconds,
        should_auto_arrange,
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
fn arrange_windows(state: State<'_, AppState>) -> Result<ArrangeWindowsResult, String> {
    let tracked = state.rejoin.tracked_pids();
    let all_roblox = snapshot_roblox_pids();
    window_manager::arrange_roblox_windows(&tracked, &all_roblox)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Acquire ROBLOX_singletonEvent and ROBLOX_singletonMutex immediately on startup
    // and hold both handles for the entire lifecycle of RoMulti.
    let mutex_manager = MutexManager::new();
    let storage = Arc::new(
        AccountStorage::new().expect("Failed to initialize RoMulti account storage"),
    );
    let api = Arc::new(
        RobloxApiClient::new().expect("Failed to initialize Roblox API client"),
    );
    let launcher = Arc::new(LaunchManager::new());
    let rejoin = Arc::new(AutoRejoinManager::new());
    let performance = Arc::new(PerformanceManager::new());

    let app_state = AppState {
        mutex_manager,
        storage,
        api,
        launcher,
        rejoin,
        performance,
    };

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            get_mutex_status,
            reinitialize_mutex,
            get_accounts,
            remove_account,
            verify_account,
            open_auth_window,
            search_games,
            launch_accounts,
            cancel_launch,
            get_auto_rejoin_config,
            set_auto_rejoin_config,
            stop_auto_rejoin,
            get_active_sessions,
            arrange_windows,
            get_performance_settings,
            apply_performance_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running RoMulti application");
}
