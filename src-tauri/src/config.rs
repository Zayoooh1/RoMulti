use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MuteMode {
    #[default]
    Off,
    MuteAll,
    MuteExceptFirst,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteGame {
    pub place_id: u64,
    pub universe_id: u64,
    pub name: String,
    pub icon_url: Option<String>,
    pub private_server_link: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchProfile {
    pub id: String,
    pub name: String,
    pub user_ids: Vec<u64>,
    pub place_id: u64,
    #[serde(default)]
    pub universe_id: Option<u64>,
    pub game_name: String,
    pub game_icon_url: Option<String>,
    pub private_server_link: Option<String>,
    pub job_id: Option<String>,
    #[serde(default)]
    pub join_same_server: bool,
    pub delay_seconds: u64,
    pub fps_cap: Option<u32>,
    #[serde(default)]
    pub low_cpu_mode: bool,
    #[serde(default)]
    pub auto_rejoin_enabled: bool,
    #[serde(default = "default_rejoin_interval")]
    pub auto_rejoin_interval_minutes: u64,
    #[serde(default)]
    pub auto_arrange: bool,
    pub monitor_id: Option<String>,
    #[serde(default)]
    pub mute_mode: MuteMode,
}

fn default_rejoin_interval() -> u64 {
    15
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default)]
    pub minimize_to_tray: bool,
    #[serde(default)]
    pub discord_webhook_url: String,
    #[serde(default = "default_true")]
    pub notify_expired_cookie: bool,
    #[serde(default = "default_true")]
    pub notify_rejoin_failed: bool,
    #[serde(default)]
    pub notify_every_rejoin: bool,
    #[serde(default)]
    pub default_monitor_id: Option<String>,
    #[serde(default)]
    pub mute_mode: MuteMode,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            minimize_to_tray: false,
            discord_webhook_url: String::new(),
            notify_expired_cookie: true,
            notify_rejoin_failed: true,
            notify_every_rejoin: false,
            default_monitor_id: None,
            mute_mode: MuteMode::Off,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default)]
    pub favorites: Vec<FavoriteGame>,
    #[serde(default)]
    pub profiles: Vec<LaunchProfile>,
    #[serde(default)]
    pub app_settings: AppSettings,
}

pub struct ConfigStorage {
    file_path: PathBuf,
    state: Mutex<AppConfig>,
}

impl ConfigStorage {
    pub fn new() -> Result<Self, String> {
        let appdata = std::env::var("APPDATA")
            .map_err(|_| "Failed to resolve %APPDATA% environment variable.".to_string())?;
        let dir = PathBuf::from(appdata).join("RoMulti");
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create config directory {}: {e}", dir.display()))?;

        let file_path = dir.join("config.json");
        let initial = read_config_file(&file_path).unwrap_or_default();

        Ok(Self {
            file_path,
            state: Mutex::new(initial),
        })
    }

    pub fn get_config(&self) -> AppConfig {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn get_app_settings(&self) -> AppSettings {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .app_settings
            .clone()
    }

    pub fn save_favorite_game(&self, mut favorite: FavoriteGame) -> Result<AppConfig, String> {
        if favorite.place_id == 0 {
            return Err("Invalid PlaceId for favorite game.".to_string());
        }
        favorite.private_server_link = favorite
            .private_server_link
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(existing) = guard
            .favorites
            .iter_mut()
            .find(|f| f.place_id == favorite.place_id)
        {
            *existing = favorite;
        } else {
            guard.favorites.push(favorite);
        }
        write_config_file(&self.file_path, &guard)?;
        Ok(guard.clone())
    }

    pub fn remove_favorite_game(&self, place_id: u64) -> Result<AppConfig, String> {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.favorites.retain(|f| f.place_id != place_id);
        write_config_file(&self.file_path, &guard)?;
        Ok(guard.clone())
    }

    pub fn save_launch_profile(&self, mut profile: LaunchProfile) -> Result<AppConfig, String> {
        let trimmed_name = profile.name.trim().to_string();
        if trimmed_name.is_empty() {
            return Err("Profile name cannot be empty.".to_string());
        }
        profile.name = trimmed_name;
        if profile.id.trim().is_empty() {
            profile.id = Uuid::new_v4().to_string();
        }
        profile.private_server_link = profile
            .private_server_link
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        profile.job_id = profile
            .job_id
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        profile.delay_seconds = profile.delay_seconds.clamp(1, 60);
        profile.auto_rejoin_interval_minutes = profile.auto_rejoin_interval_minutes.clamp(1, 240);

        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(existing) = guard.profiles.iter_mut().find(|p| p.id == profile.id) {
            *existing = profile;
        } else {
            guard.profiles.push(profile);
        }
        write_config_file(&self.file_path, &guard)?;
        Ok(guard.clone())
    }

    pub fn delete_launch_profile(&self, id: &str) -> Result<AppConfig, String> {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.profiles.retain(|p| p.id != id);
        write_config_file(&self.file_path, &guard)?;
        Ok(guard.clone())
    }

    pub fn update_app_settings(&self, mut settings: AppSettings) -> Result<AppConfig, String> {
        settings.discord_webhook_url = settings.discord_webhook_url.trim().to_string();
        settings.default_monitor_id = settings
            .default_monitor_id
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.app_settings = settings;
        write_config_file(&self.file_path, &guard)?;
        Ok(guard.clone())
    }

    pub fn set_mute_mode(&self, mode: MuteMode) -> Result<AppConfig, String> {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.app_settings.mute_mode = mode;
        write_config_file(&self.file_path, &guard)?;
        Ok(guard.clone())
    }
}

fn read_config_file(path: &PathBuf) -> Result<AppConfig, String> {
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let raw =
        fs::read_to_string(path).map_err(|e| format!("Failed to read config.json: {e}"))?;
    if raw.trim().is_empty() {
        return Ok(AppConfig::default());
    }
    serde_json::from_str::<AppConfig>(&raw)
        .map_err(|e| format!("Failed to parse config.json: {e}"))
}

fn write_config_file(path: &PathBuf, data: &AppConfig) -> Result<(), String> {
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| format!("Failed to serialize config.json: {e}"))?;

    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, &json)
        .map_err(|e| format!("Failed to write temporary config file: {e}"))?;

    if let Err(rename_err) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        fs::write(path, &json).map_err(|write_err| {
            format!("Failed to save config.json (rename: {rename_err}, write: {write_err})")
        })?;
    }

    Ok(())
}
