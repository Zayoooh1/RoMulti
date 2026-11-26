use crate::config::AppSettings;
use reqwest::Client;
use serde_json::json;
use std::time::Duration;

const COLOR_EMERALD: u32 = 0x10B981;
const COLOR_AMBER: u32 = 0xF59E0B;
const COLOR_RED: u32 = 0xEF4444;

pub struct NotificationManager {
    client: Client,
}

impl NotificationManager {
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("Failed to initialize notification HTTP client: {e}"))?;
        Ok(Self { client })
    }

    pub fn notify_expired_cookie(&self, settings: &AppSettings, username: &str, details: &str) {
        if !settings.notify_expired_cookie {
            return;
        }
        let Ok(url) = validate_discord_webhook_url(&settings.discord_webhook_url) else {
            return;
        };

        let client = self.client.clone();
        let username = username.to_string();
        let details = details.to_string();

        tauri::async_runtime::spawn(async move {
            let _ = send_discord_embed(
                &client,
                &url,
                "RoMulti — Session Cookie Expired",
                &format!(
                    "The `.ROBLOSECURITY` session for **@{username}** was rejected or expired. Please re-add this account in RoMulti."
                ),
                COLOR_RED,
                vec![
                    ("Account", format!("@{username}"), true),
                    ("Details", truncate_field(&details, 500), false),
                ],
            )
            .await;
        });
    }

    pub fn notify_rejoin_failed(
        &self,
        settings: &AppSettings,
        username: &str,
        place_id: u64,
        last_error: &str,
    ) {
        if !settings.notify_rejoin_failed {
            return;
        }
        let Ok(url) = validate_discord_webhook_url(&settings.discord_webhook_url) else {
            return;
        };

        let client = self.client.clone();
        let username = username.to_string();
        let last_error = last_error.to_string();

        tauri::async_runtime::spawn(async move {
            let _ = send_discord_embed(
                &client,
                &url,
                "RoMulti — Auto Rejoin Paused (3 Failed Attempts)",
                &format!(
                    "Auto Rejoin for **@{username}** was paused after 3 consecutive failures."
                ),
                COLOR_RED,
                vec![
                    ("Account", format!("@{username}"), true),
                    ("PlaceId", place_id.to_string(), true),
                    ("Last Error", truncate_field(&last_error, 500), false),
                ],
            )
            .await;
        });
    }

    pub fn notify_rejoin_triggered(
        &self,
        settings: &AppSettings,
        username: &str,
        place_id: u64,
        reason: &str,
        attempt: u32,
    ) {
        if !settings.notify_every_rejoin {
            return;
        }
        let Ok(url) = validate_discord_webhook_url(&settings.discord_webhook_url) else {
            return;
        };

        let client = self.client.clone();
        let username = username.to_string();
        let reason = reason.to_string();

        tauri::async_runtime::spawn(async move {
            let _ = send_discord_embed(
                &client,
                &url,
                "RoMulti — Auto Rejoin Triggered",
                &format!("Rejoining **@{username}** (attempt {attempt}/3)."),
                COLOR_AMBER,
                vec![
                    ("Account", format!("@{username}"), true),
                    ("PlaceId", place_id.to_string(), true),
                    ("Reason", truncate_field(&reason, 500), false),
                ],
            )
            .await;
        });
    }

    pub async fn send_test_webhook(&self, raw_url: &str) -> Result<(), String> {
        let url = validate_discord_webhook_url(raw_url)?;
        send_discord_embed(
            &self.client,
            &url,
            "RoMulti — Test Notification",
            "Your Discord Webhook is connected and ready to receive RoMulti alerts.",
            COLOR_EMERALD,
            vec![("Status", "Connected".to_string(), true)],
        )
        .await
    }
}

pub fn validate_discord_webhook_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Discord Webhook URL is empty.".to_string());
    }
    let valid_prefix = trimmed.starts_with("https://discord.com/api/webhooks/")
        || trimmed.starts_with("https://discordapp.com/api/webhooks/");
    if !valid_prefix {
        return Err(
            "Invalid Discord Webhook URL. Expected https://discord.com/api/webhooks/... or https://discordapp.com/api/webhooks/..."
                .to_string(),
        );
    }
    Ok(trimmed.to_string())
}

async fn send_discord_embed(
    client: &Client,
    webhook_url: &str,
    title: &str,
    description: &str,
    color: u32,
    fields: Vec<(&str, String, bool)>,
) -> Result<(), String> {
    let embed_fields: Vec<serde_json::Value> = fields
        .into_iter()
        .map(|(name, value, inline)| {
            json!({
                "name": name,
                "value": value,
                "inline": inline,
            })
        })
        .collect();

    let payload = json!({
        "username": "RoMulti",
        "embeds": [
            {
                "title": title,
                "description": description,
                "color": color,
                "fields": embed_fields,
                "footer": {
                    "text": "RoMulti v1.1"
                }
            }
        ]
    });

    let resp = client
        .post(webhook_url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to reach Discord Webhook: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "Discord Webhook returned HTTP {}. Verify the webhook URL is valid.",
            resp.status()
        ));
    }

    Ok(())
}

fn truncate_field(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        input.to_string()
    } else {
        let mut s: String = input.chars().take(max_chars.saturating_sub(3)).collect();
        s.push_str("...");
        s
    }
}
