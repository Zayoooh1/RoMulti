use crate::roblox_api::RobloxApiClient;
use crate::storage::{AccountStorage, PublicAccount};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tokio::sync::oneshot;

#[cfg(windows)]
use webview2_com::{
    GetCookiesCompletedHandler,
    Microsoft::Web::WebView2::Win32::{ICoreWebView2CookieManager, ICoreWebView2_2},
};
#[cfg(windows)]
use windows::{
    core::{w, Interface, PWSTR},
    Win32::System::Com::CoTaskMemFree,
};

const AUTH_WINDOW_LABEL: &str = "roblox-auth";
const ROBLOX_LOGIN_URL: &str = "https://www.roblox.com/login";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatusPayload {
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthErrorPayload {
    pub error: String,
}

pub async fn open_roblox_auth_window(
    app: AppHandle,
    storage: Arc<AccountStorage>,
    api: Arc<RobloxApiClient>,
) -> Result<(), String> {
    // If login window is already open, bring it to front instead of opening duplicates.
    if let Some(existing) = app.get_webview_window(AUTH_WINDOW_LABEL) {
        let _ = existing.unminimize();
        let _ = existing.set_focus();
        return Ok(());
    }

    // Best-effort cleanup of previous isolated WebView2 session directories.
    let sessions_root = auth_sessions_root();
    cleanup_old_sessions(&sessions_root);

    let session_stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let session_dir = sessions_root.join(format!("session_{session_stamp}"));
    let _ = fs::create_dir_all(&session_dir);

    let login_url: url::Url = ROBLOX_LOGIN_URL
        .parse()
        .map_err(|e| format!("Invalid Roblox login URL: {e}"))?;

    let mut builder = WebviewWindowBuilder::new(&app, AUTH_WINDOW_LABEL, WebviewUrl::External(login_url))
        .title("Sign in to Roblox — RoMulti")
        .inner_size(460.0, 680.0)
        .min_inner_size(400.0, 560.0)
        .center()
        .resizable(true);

    #[cfg(windows)]
    {
        builder = builder.data_directory(session_dir.clone());
    }

    let auth_window = builder
        .build()
        .map_err(|e| format!("Failed to open Roblox login window: {e}"))?;

    let _ = auth_window.set_focus();
    let _ = app.emit(
        "auth-status",
        AuthStatusPayload {
            message: "Waiting for Roblox sign-in...".to_string(),
        },
    );

    // Spawn polling task to detect .ROBLOSECURITY cookie natively via WebView2 CookieManager.
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(500));

        loop {
            interval.tick().await;

            let Some(win) = app.get_webview_window(AUTH_WINDOW_LABEL) else {
                let _ = app.emit("auth-cancelled", ());
                schedule_session_cleanup(session_dir);
                return;
            };

            match poll_roblosecurity_cookie(&win).await {
                Ok(Some(cookie)) if !cookie.trim().is_empty() => {
                    let _ = app.emit(
                        "auth-status",
                        AuthStatusPayload {
                            message: "Cookie detected. Fetching Roblox profile...".to_string(),
                        },
                    );

                    match finalize_account_login(&app, &win, &storage, &api, &cookie).await {
                        Ok(account) => {
                            let _ = app.emit("account-added", account);
                            schedule_session_cleanup(session_dir);
                            return;
                        }
                        Err(err) => {
                            let _ = clear_webview_cookies(&win).await;
                            let _ = win.close();
                            let _ = app.emit("auth-error", AuthErrorPayload { error: err });
                            schedule_session_cleanup(session_dir);
                            return;
                        }
                    }
                }
                Ok(_) => {
                    // Cookie not present yet; keep polling until user logs in or closes window.
                }
                Err(_) => {
                    // WebView2 might still be initializing on first ticks; continue polling.
                }
            }
        }
    });

    Ok(())
}

async fn finalize_account_login(
    _app: &AppHandle,
    win: &WebviewWindow,
    storage: &Arc<AccountStorage>,
    api: &Arc<RobloxApiClient>,
    cookie: &str,
) -> Result<PublicAccount, String> {
    let user = api.fetch_authenticated_user(cookie).await?;
    let avatar_url = api
        .fetch_user_avatar_headshot(user.id)
        .await
        .unwrap_or_default();

    let saved = storage.upsert_account(
        user.id,
        user.name,
        user.display_name,
        avatar_url,
        cookie,
    )?;

    // Clear all cookies inside the auth WebView2 profile before closing the window.
    let _ = clear_webview_cookies(win).await;
    tokio::time::sleep(Duration::from_millis(120)).await;
    let _ = win.close();

    Ok(saved)
}

#[cfg(windows)]
async fn poll_roblosecurity_cookie(window: &WebviewWindow) -> Result<Option<String>, String> {
    let (tx, rx) = oneshot::channel::<Result<Option<String>, String>>();

    let dispatch_res = window.with_webview(move |webview| {
        let mut tx = Some(tx);

        let setup_result: Result<(), String> = (|| unsafe {
            let core = webview
                .controller()
                .CoreWebView2()
                .map_err(|e| format!("CoreWebView2 unavailable: {e}"))?;
            let core2: ICoreWebView2_2 = core
                .cast()
                .map_err(|e| format!("ICoreWebView2_2 cast failed: {e}"))?;
            let cookie_manager: ICoreWebView2CookieManager = core2
                .CookieManager()
                .map_err(|e| format!("CookieManager unavailable: {e}"))?;

            let mut callback_tx = tx.take();
            let handler = GetCookiesCompletedHandler::create(Box::new(move |result, list| {
                let mut found: Option<String> = None;
                let mut err_msg: Option<String> = None;

                if let Err(e) = result {
                    err_msg = Some(format!("GetCookies callback error: {e}"));
                } else if let Some(cookie_list) = list {
                    let mut count: u32 = 0;
                    if cookie_list.Count(&mut count).is_ok() {
                        for idx in 0..count {
                            if let Ok(cookie) = cookie_list.GetValueAtIndex(idx) {
                                let mut name_ptr = PWSTR::null();
                                if cookie.Name(&mut name_ptr).is_ok() && !name_ptr.is_null() {
                                    let name = name_ptr.to_string().unwrap_or_default();
                                    CoTaskMemFree(Some(name_ptr.0 as *const std::ffi::c_void));

                                    if name == ".ROBLOSECURITY" {
                                        let mut val_ptr = PWSTR::null();
                                        if cookie.Value(&mut val_ptr).is_ok() && !val_ptr.is_null()
                                        {
                                            let val = val_ptr.to_string().unwrap_or_default();
                                            CoTaskMemFree(Some(
                                                val_ptr.0 as *const std::ffi::c_void,
                                            ));
                                            if !val.is_empty() {
                                                found = Some(val);
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(sender) = callback_tx.take() {
                    let _ = match err_msg {
                        Some(msg) => sender.send(Err(msg)),
                        None => sender.send(Ok(found)),
                    };
                }
                Ok(())
            }));

            cookie_manager
                .GetCookies(w!("https://www.roblox.com"), &handler)
                .map_err(|e| format!("GetCookies call failed: {e}"))?;

            Ok(())
        })();

        if let Err(err) = setup_result {
            if let Some(sender) = tx.take() {
                let _ = sender.send(Err(err));
            }
        }
    });

    if let Err(e) = dispatch_res {
        return Err(format!("with_webview dispatch failed: {e}"));
    }

    match tokio::time::timeout(Duration::from_secs(3), rx).await {
        Ok(Ok(res)) => res,
        Ok(Err(_)) => Err("Cookie polling channel closed unexpectedly.".to_string()),
        Err(_) => Err("Timed out waiting for WebView2 cookie manager.".to_string()),
    }
}

#[cfg(not(windows))]
async fn poll_roblosecurity_cookie(_window: &WebviewWindow) -> Result<Option<String>, String> {
    Err("WebView2 cookie capture is only supported on Windows.".to_string())
}

#[cfg(windows)]
async fn clear_webview_cookies(window: &WebviewWindow) -> Result<(), String> {
    let (tx, rx) = oneshot::channel::<Result<(), String>>();

    let dispatch_res = window.with_webview(move |webview| {
        let res = (|| unsafe {
            let core = webview
                .controller()
                .CoreWebView2()
                .map_err(|e| format!("CoreWebView2 unavailable: {e}"))?;
            let core2: ICoreWebView2_2 = core
                .cast()
                .map_err(|e| format!("ICoreWebView2_2 cast failed: {e}"))?;
            let cookie_manager: ICoreWebView2CookieManager = core2
                .CookieManager()
                .map_err(|e| format!("CookieManager unavailable: {e}"))?;
            cookie_manager
                .DeleteAllCookies()
                .map_err(|e| format!("DeleteAllCookies failed: {e}"))?;
            Ok(())
        })();

        let _ = tx.send(res);
    });

    if let Err(e) = dispatch_res {
        return Err(format!("with_webview cookie clear failed: {e}"));
    }

    match tokio::time::timeout(Duration::from_secs(2), rx).await {
        Ok(Ok(r)) => r,
        _ => Ok(()),
    }
}

#[cfg(not(windows))]
async fn clear_webview_cookies(_window: &WebviewWindow) -> Result<(), String> {
    Ok(())
}

fn auth_sessions_root() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("TEMP"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base).join("RoMulti").join("auth_sessions")
}

fn cleanup_old_sessions(root: &PathBuf) {
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let _ = fs::remove_dir_all(path);
            }
        }
    }
}

fn schedule_session_cleanup(dir: PathBuf) {
    tauri::async_runtime::spawn(async move {
        // Wait for WebView2 subprocesses to release locks on the temporary profile directory.
        tokio::time::sleep(Duration::from_secs(3)).await;
        let _ = fs::remove_dir_all(dir);
    });
}
