use crate::config::MuteMode;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[cfg(windows)]
use windows::{
    core::Interface,
    Win32::{
        Foundation::{RPC_E_CHANGED_MODE, S_FALSE, S_OK},
        Media::Audio::{
            eConsole, eRender, IAudioSessionControl2, IAudioSessionEnumerator,
            IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator,
        },
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
        },
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioApplyResult {
    pub mode: MuteMode,
    pub muted_count: usize,
    pub unmuted_count: usize,
}

/// Applies the selected `MuteMode` across all tracked and running Roblox instances via Windows WASAPI.
#[cfg(windows)]
pub fn apply_mute_mode(
    mode: MuteMode,
    tracked_pids_in_order: &[u32],
    all_roblox_pids: &HashSet<u32>,
) -> Result<AudioApplyResult, String> {
    let mut all_targets: HashSet<u32> = all_roblox_pids.clone();
    all_targets.extend(tracked_pids_in_order.iter().copied());
    all_targets.remove(&0);

    if all_targets.is_empty() {
        return Ok(AudioApplyResult {
            mode,
            muted_count: 0,
            unmuted_count: 0,
        });
    }

    let keep_unmuted_pid: Option<u32> = match mode {
        MuteMode::MuteExceptFirst => tracked_pids_in_order
            .iter()
            .copied()
            .find(|&p| p != 0)
            .or_else(|| all_targets.iter().copied().min()),
        _ => None,
    };

    let mut desired: HashMap<u32, bool> = HashMap::with_capacity(all_targets.len());
    for pid in all_targets {
        let should_mute = match mode {
            MuteMode::Off => false,
            MuteMode::MuteAll => true,
            MuteMode::MuteExceptFirst => Some(pid) != keep_unmuted_pid,
        };
        desired.insert(pid, should_mute);
    }

    let (muted_count, unmuted_count) = apply_pid_mute_map(&desired)?;

    Ok(AudioApplyResult {
        mode,
        muted_count,
        unmuted_count,
    })
}

#[cfg(not(windows))]
pub fn apply_mute_mode(
    mode: MuteMode,
    _tracked_pids_in_order: &[u32],
    _all_roblox_pids: &HashSet<u32>,
) -> Result<AudioApplyResult, String> {
    Ok(AudioApplyResult {
        mode,
        muted_count: 0,
        unmuted_count: 0,
    })
}

/// Mutes or unmutes all WASAPI audio sessions belonging to a single process `pid`.
/// Returns `Ok(true)` if at least one audio session for `pid` was updated.
#[cfg(windows)]
pub fn set_pid_muted(pid: u32, muted: bool) -> Result<bool, String> {
    if pid == 0 {
        return Ok(false);
    }
    let mut map = HashMap::new();
    map.insert(pid, muted);
    let (m, u) = apply_pid_mute_map(&map)?;
    Ok(m + u > 0)
}

#[cfg(not(windows))]
pub fn set_pid_muted(_pid: u32, _muted: bool) -> Result<bool, String> {
    Ok(false)
}

#[cfg(windows)]
struct ComGuard {
    should_uninit: bool,
}

#[cfg(windows)]
impl ComGuard {
    fn init() -> Result<Self, String> {
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr == S_OK || hr == S_FALSE {
            Ok(Self {
                should_uninit: true,
            })
        } else if hr == RPC_E_CHANGED_MODE {
            // Thread already initialized COM in another apartment mode; safe to proceed without uninitializing.
            Ok(Self {
                should_uninit: false,
            })
        } else {
            Err(format!("COM initialization failed: {hr:?}"))
        }
    }
}

#[cfg(windows)]
impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.should_uninit {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

#[cfg(windows)]
fn apply_pid_mute_map(desired: &HashMap<u32, bool>) -> Result<(usize, usize), String> {
    if desired.is_empty() {
        return Ok((0, 0));
    }

    let _com = ComGuard::init()?;

    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("Failed to create IMMDeviceEnumerator: {e}"))?;

        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| format!("Failed to get default audio render endpoint: {e}"))?;

        let manager: IAudioSessionManager2 = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| format!("Failed to activate IAudioSessionManager2: {e}"))?;

        let session_enum: IAudioSessionEnumerator = manager
            .GetSessionEnumerator()
            .map_err(|e| format!("Failed to get audio session enumerator: {e}"))?;

        let count = session_enum
            .GetCount()
            .map_err(|e| format!("Failed to get audio session count: {e}"))?;

        let mut muted_count = 0usize;
        let mut unmuted_count = 0usize;

        for idx in 0..count {
            let Ok(control) = session_enum.GetSession(idx) else {
                continue;
            };
            let Ok(control2) = control.cast::<IAudioSessionControl2>() else {
                continue;
            };
            let Ok(session_pid) = control2.GetProcessId() else {
                continue;
            };

            let Some(&should_mute) = desired.get(&session_pid) else {
                continue;
            };

            let Ok(volume) = control.cast::<ISimpleAudioVolume>() else {
                continue;
            };

            if volume.SetMute(should_mute, std::ptr::null()).is_ok() {
                if should_mute {
                    muted_count += 1;
                } else {
                    unmuted_count += 1;
                }
            }
        }

        Ok((muted_count, unmuted_count))
    }
}
