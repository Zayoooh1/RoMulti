use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

#[cfg(windows)]
use windows::Win32::{
    Foundation::CloseHandle,
    System::Threading::{
        OpenProcess, SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
        PROCESS_SET_INFORMATION,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceSettings {
    /// None = Default (remove DFIntTaskSchedulerTargetFps), or explicit cap e.g. 15, 30, 60, 240.
    pub fps_cap: Option<u32>,
    /// Low CPU Mode: BELOW_NORMAL_PRIORITY_CLASS on Roblox processes + lightweight ClientAppSettings FFlags.
    pub low_cpu_mode: bool,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            fps_cap: None,
            low_cpu_mode: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPerformanceResult {
    pub settings: PerformanceSettings,
    pub client_settings_path: Option<String>,
    pub updated_processes: usize,
    pub warning: Option<String>,
}

pub struct PerformanceManager {
    current: Mutex<PerformanceSettings>,
}

impl PerformanceManager {
    pub fn new() -> Self {
        let initial = read_existing_settings().unwrap_or_default();
        Self {
            current: Mutex::new(initial),
        }
    }

    pub fn get_settings(&self) -> PerformanceSettings {
        self.current
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn is_low_cpu_enabled(&self) -> bool {
        self.current
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .low_cpu_mode
    }

    pub fn apply(
        &self,
        settings: PerformanceSettings,
        target_pids: &HashSet<u32>,
    ) -> Result<ApplyPerformanceResult, String> {
        {
            let mut guard = self.current.lock().unwrap_or_else(|e| e.into_inner());
            *guard = settings.clone();
        }

        // 1. Apply Win32 process priority to all tracked/running Roblox processes
        let mut updated_processes = 0usize;
        for &pid in target_pids {
            if apply_priority_to_pid(pid, settings.low_cpu_mode).is_ok() {
                updated_processes += 1;
            }
        }

        // 2. Locate active Roblox Versions folder and update ClientAppSettings.json
        let mut warning = None;
        let client_settings_path = match find_active_roblox_version_dir() {
            Some(version_dir) => {
                let settings_dir = version_dir.join("ClientSettings");
                fs::create_dir_all(&settings_dir).map_err(|e| {
                    format!(
                        "Failed to create ClientSettings folder at {}: {e}",
                        settings_dir.display()
                    )
                })?;
                let file_path = settings_dir.join("ClientAppSettings.json");
                write_client_app_settings(&file_path, &settings)?;
                Some(file_path.to_string_lossy().to_string())
            }
            None => {
                warning = Some(
                    "Roblox installation folder (%LOCALAPPDATA%\\Roblox\\Versions) with RobloxPlayerBeta.exe was not found. Process priority was applied where possible, but ClientAppSettings.json could not be written."
                        .to_string(),
                );
                None
            }
        };

        Ok(ApplyPerformanceResult {
            settings,
            client_settings_path,
            updated_processes,
            warning,
        })
    }
}

/// Applies BELOW_NORMAL_PRIORITY_CLASS (when `low_cpu` is true) or NORMAL_PRIORITY_CLASS
/// to a single Roblox process by PID.
#[cfg(windows)]
pub fn apply_priority_to_pid(pid: u32, low_cpu: bool) -> Result<(), String> {
    if pid == 0 {
        return Ok(());
    }
    unsafe {
        let handle = OpenProcess(PROCESS_SET_INFORMATION, false, pid)
            .map_err(|e| format!("OpenProcess(PROCESS_SET_INFORMATION) failed for PID {pid}: {e}"))?;
        let priority = if low_cpu {
            BELOW_NORMAL_PRIORITY_CLASS
        } else {
            NORMAL_PRIORITY_CLASS
        };
        let res = SetPriorityClass(handle, priority)
            .map_err(|e| format!("SetPriorityClass failed for PID {pid}: {e}"));
        let _ = CloseHandle(handle);
        res
    }
}

#[cfg(not(windows))]
pub fn apply_priority_to_pid(_pid: u32, _low_cpu: bool) -> Result<(), String> {
    Ok(())
}

/// Finds the newest Roblox Versions subdirectory containing `RobloxPlayerBeta.exe`.
fn find_active_roblox_version_dir() -> Option<PathBuf> {
    let mut candidate_roots = Vec::new();
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        candidate_roots.push(PathBuf::from(local_app_data).join("Roblox").join("Versions"));
    }
    if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
        candidate_roots.push(PathBuf::from(pf86).join("Roblox").join("Versions"));
    }
    if let Ok(pf) = std::env::var("ProgramFiles") {
        candidate_roots.push(PathBuf::from(pf).join("Roblox").join("Versions"));
    }

    let mut best: Option<(SystemTime, PathBuf)> = None;

    for root in candidate_roots {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let exe = path.join("RobloxPlayerBeta.exe");
            if !exe.is_file() {
                continue;
            }
            let modified = fs::metadata(&exe)
                .and_then(|m| m.modified())
                .or_else(|_| fs::metadata(&path).and_then(|m| m.modified()))
                .unwrap_or(SystemTime::UNIX_EPOCH);

            match &best {
                Some((best_time, _)) if modified <= *best_time => {}
                _ => best = Some((modified, path)),
            }
        }
    }

    best.map(|(_, path)| path)
}

fn read_existing_settings() -> Option<PerformanceSettings> {
    let dir = find_active_roblox_version_dir()?;
    let file_path = dir.join("ClientSettings").join("ClientAppSettings.json");
    let raw = fs::read_to_string(file_path).ok()?;
    let json: Value = serde_json::from_str(&raw).ok()?;
    let obj = json.as_object()?;

    let fps_cap = obj
        .get("DFIntTaskSchedulerTargetFps")
        .and_then(Value::as_u64)
        .map(|v| v as u32);
    let low_cpu_mode = obj
        .get("FIntRenderShadowIntensity")
        .and_then(Value::as_i64)
        .map(|v| v == 0)
        .unwrap_or(false);

    Some(PerformanceSettings {
        fps_cap,
        low_cpu_mode,
    })
}

fn write_client_app_settings(path: &Path, settings: &PerformanceSettings) -> Result<(), String> {
    let mut map: Map<String, Value> = if path.exists() {
        fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    } else {
        Map::new()
    };

    // FPS Cap management
    if let Some(fps) = settings.fps_cap {
        map.insert("DFIntTaskSchedulerTargetFps".to_string(), Value::from(fps));
    } else if settings.low_cpu_mode {
        // Low CPU Mode defaults to 15 FPS when no explicit cap is selected
        map.insert("DFIntTaskSchedulerTargetFps".to_string(), Value::from(15));
    } else {
        map.remove("DFIntTaskSchedulerTargetFps");
    }

    // Safe Low CPU Mode FFlags (cleanly removed when toggled off)
    if settings.low_cpu_mode {
        map.insert("FIntRenderShadowIntensity".to_string(), Value::from(0));
        map.insert(
            "DFFlagTextureQualityOverrideEnabled".to_string(),
            Value::from(true),
        );
        map.insert("DFIntTextureQualityOverride".to_string(), Value::from(1));
    } else {
        map.remove("FIntRenderShadowIntensity");
        map.remove("DFFlagTextureQualityOverrideEnabled");
        map.remove("DFIntTextureQualityOverride");
    }

    let serialized = serde_json::to_string_pretty(&Value::Object(map))
        .map_err(|e| format!("Failed to serialize ClientAppSettings.json: {e}"))?;

    fs::write(path, serialized)
        .map_err(|e| format!("Failed to write {}: {e}", path.display()))?;
    Ok(())
}
