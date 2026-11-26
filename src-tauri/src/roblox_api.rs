use reqwest::{
    header::{HeaderMap, HeaderValue, CONTENT_LENGTH, CONTENT_TYPE, COOKIE, ORIGIN, REFERER, USER_AGENT},
    Client, StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use uuid::Uuid;

const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticatedUser {
    pub id: u64,
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPresence {
    pub user_presence_type: u8,
    pub place_id: Option<u64>,
    pub root_place_id: Option<u64>,
    pub last_location: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PresenceApiResponse {
    #[serde(rename = "userPresences")]
    user_presences: Vec<UserPresence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSearchResult {
    pub universe_id: u64,
    pub place_id: u64,
    pub name: String,
    pub creator_name: String,
    pub playing: u64,
    pub visits: u64,
    pub max_players: u64,
    pub icon_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ThumbnailItem {
    #[serde(rename = "targetId")]
    target_id: u64,
    #[serde(rename = "imageUrl")]
    image_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ThumbnailResponse {
    data: Vec<ThumbnailItem>,
}

#[derive(Debug, Deserialize)]
struct PlaceUniverseResponse {
    #[serde(rename = "universeId")]
    universe_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GamesListResponse {
    data: Vec<GameDetailItem>,
}

#[derive(Debug, Deserialize)]
struct GameCreator {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameDetailItem {
    id: u64,
    root_place_id: u64,
    name: String,
    creator: Option<GameCreator>,
    playing: Option<u64>,
    visits: Option<u64>,
    max_players: Option<u64>,
}

struct OmniCandidate {
    universe_id: u64,
    root_place_id: u64,
    name: String,
    creator_name: String,
    playing: u64,
}

pub struct RobloxApiClient {
    client: Client,
}

impl RobloxApiClient {
    pub fn new() -> Result<Self, String> {
        let mut default_headers = HeaderMap::new();
        default_headers.insert(USER_AGENT, HeaderValue::from_static(BROWSER_USER_AGENT));

        let client = Client::builder()
            .default_headers(default_headers)
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| format!("Failed to initialize HTTP client: {e}"))?;

        Ok(Self { client })
    }

    pub async fn fetch_authenticated_user(&self, cookie: &str) -> Result<AuthenticatedUser, String> {
        let resp = self
            .client
            .get("https://users.roblox.com/v1/users/authenticated")
            .header(COOKIE, format!(".ROBLOSECURITY={cookie}"))
            .send()
            .await
            .map_err(|e| format!("Network error contacting Roblox Users API: {e}"))?;

        let status = resp.status();
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err("Session cookie (.ROBLOSECURITY) is expired or invalid. Please re-add this account.".to_string());
        }
        if !status.is_success() {
            return Err(format!("Roblox Users API returned HTTP {status}."));
        }

        resp.json::<AuthenticatedUser>()
            .await
            .map_err(|e| format!("Failed to parse Roblox user response: {e}"))
    }

    pub async fn fetch_user_avatar_headshot(&self, user_id: u64) -> Result<String, String> {
        let url = format!(
            "https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={user_id}&size=150x150&format=Png&isCircular=false"
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch user avatar thumbnail: {e}"))?;

        if !resp.status().is_success() {
            return Ok(String::new());
        }

        let parsed = resp
            .json::<ThumbnailResponse>()
            .await
            .map_err(|e| format!("Failed to parse avatar thumbnail response: {e}"))?;

        Ok(parsed
            .data
            .into_iter()
            .next()
            .and_then(|item| item.image_url)
            .unwrap_or_default())
    }

    pub async fn search_games(&self, query: &str) -> Result<Vec<GameSearchResult>, String> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        // Check for direct numeric PlaceId or roblox.com/games/{placeId} URL
        if let Some(place_id) = extract_place_id(trimmed) {
            return self.lookup_game_by_place_id(place_id).await;
        }

        // Perform omni-search by game name
        let session_id = Uuid::new_v4().to_string();
        let encoded_query = urlencoding::encode(trimmed);
        let omni_url = format!(
            "https://apis.roblox.com/search-api/omni-search?searchQuery={encoded_query}&sessionId={session_id}&pageType=all"
        );

        let resp = self
            .client
            .get(&omni_url)
            .send()
            .await
            .map_err(|e| format!("Network error searching Roblox games: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!(
                "Roblox search API returned HTTP {}. Try searching by PlaceId instead.",
                resp.status()
            ));
        }

        let json: Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Roblox search response: {e}"))?;

        let candidates = extract_omni_candidates(&json, 24);
        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        let universe_ids: Vec<u64> = candidates.iter().map(|c| c.universe_id).collect();
        let (details_map, icons_map) = tokio::join!(
            self.fetch_game_details(&universe_ids),
            self.fetch_game_icons(&universe_ids)
        );
        let details_map = details_map.unwrap_or_default();
        let icons_map = icons_map.unwrap_or_default();

        let results = candidates
            .into_iter()
            .map(|cand| {
                let icon_url = icons_map.get(&cand.universe_id).cloned();
                if let Some(detail) = details_map.get(&cand.universe_id) {
                    GameSearchResult {
                        universe_id: detail.id,
                        place_id: if detail.root_place_id > 0 {
                            detail.root_place_id
                        } else {
                            cand.root_place_id
                        },
                        name: detail.name.clone(),
                        creator_name: detail
                            .creator
                            .as_ref()
                            .and_then(|c| c.name.clone())
                            .unwrap_or(cand.creator_name),
                        playing: detail.playing.unwrap_or(cand.playing),
                        visits: detail.visits.unwrap_or(0),
                        max_players: detail.max_players.unwrap_or(0),
                        icon_url,
                    }
                } else {
                    GameSearchResult {
                        universe_id: cand.universe_id,
                        place_id: cand.root_place_id,
                        name: cand.name,
                        creator_name: cand.creator_name,
                        playing: cand.playing,
                        visits: 0,
                        max_players: 0,
                        icon_url,
                    }
                }
            })
            .filter(|g| g.place_id > 0)
            .collect();

        Ok(results)
    }

    async fn lookup_game_by_place_id(&self, place_id: u64) -> Result<Vec<GameSearchResult>, String> {
        let universe_url = format!("https://apis.roblox.com/universes/v1/places/{place_id}/universe");
        let resp = self
            .client
            .get(&universe_url)
            .send()
            .await
            .map_err(|e| format!("Network error looking up PlaceId {place_id}: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!(
                "Could not resolve PlaceId {place_id} (HTTP {}). Verify the PlaceId is valid.",
                resp.status()
            ));
        }

        let parsed = resp
            .json::<PlaceUniverseResponse>()
            .await
            .map_err(|e| format!("Failed to parse universe lookup response: {e}"))?;

        let universe_id = parsed
            .universe_id
            .filter(|&id| id > 0)
            .ok_or_else(|| format!("No Roblox experience found for PlaceId {place_id}."))?;

        let ids = [universe_id];
        let (details_res, icons_res) = tokio::join!(
            self.fetch_game_details(&ids),
            self.fetch_game_icons(&ids)
        );
        let details_map = details_res?;
        let icons_map = icons_res.unwrap_or_default();

        let detail = details_map
            .get(&universe_id)
            .ok_or_else(|| format!("Game details unavailable for PlaceId {place_id}."))?;

        Ok(vec![GameSearchResult {
            universe_id,
            // Keep the exact PlaceId entered by the user (supports sub-places as well as root places)
            place_id,
            name: detail.name.clone(),
            creator_name: detail
                .creator
                .as_ref()
                .and_then(|c| c.name.clone())
                .unwrap_or_else(|| "Unknown".to_string()),
            playing: detail.playing.unwrap_or(0),
            visits: detail.visits.unwrap_or(0),
            max_players: detail.max_players.unwrap_or(0),
            icon_url: icons_map.get(&universe_id).cloned(),
        }])
    }

    async fn fetch_game_details(
        &self,
        universe_ids: &[u64],
    ) -> Result<HashMap<u64, GameDetailItem>, String> {
        if universe_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let joined = universe_ids
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let url = format!("https://games.roblox.com/v1/games?universeIds={joined}");

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch game details: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Roblox Games API returned HTTP {}.", resp.status()));
        }

        let parsed = resp
            .json::<GamesListResponse>()
            .await
            .map_err(|e| format!("Failed to parse game details response: {e}"))?;

        Ok(parsed.data.into_iter().map(|g| (g.id, g)).collect())
    }

    async fn fetch_game_icons(
        &self,
        universe_ids: &[u64],
    ) -> Result<HashMap<u64, String>, String> {
        if universe_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let joined = universe_ids
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let url = format!(
            "https://thumbnails.roblox.com/v1/games/icons?universeIds={joined}&returnPolicy=PlaceHolder&size=150x150&format=Png&isCircular=false"
        );

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch game icons: {e}"))?;

        if !resp.status().is_success() {
            return Ok(HashMap::new());
        }

        let parsed = resp
            .json::<ThumbnailResponse>()
            .await
            .map_err(|e| format!("Failed to parse game icons response: {e}"))?;

        let mut map = HashMap::new();
        for item in parsed.data {
            if let Some(img) = item.image_url {
                if !img.is_empty() {
                    map.insert(item.target_id, img);
                }
            }
        }
        Ok(map)
    }

    pub async fn generate_auth_ticket(&self, cookie: &str) -> Result<String, String> {
        let url = "https://auth.roblox.com/v1/authentication-ticket";
        let cookie_header = format!(".ROBLOSECURITY={cookie}");

        // First POST to obtain x-csrf-token (Roblox returns 403 with x-csrf-token header)
        let first_resp = self
            .client
            .post(url)
            .header(COOKIE, &cookie_header)
            .header(REFERER, "https://www.roblox.com/")
            .header(ORIGIN, "https://www.roblox.com")
            .header(CONTENT_TYPE, "application/json")
            .header(CONTENT_LENGTH, "0")
            .send()
            .await
            .map_err(|e| format!("Network error requesting auth ticket: {e}"))?;

        let final_resp = if first_resp.status() == StatusCode::FORBIDDEN {
            let csrf_token = first_resp
                .headers()
                .get("x-csrf-token")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
                .ok_or_else(|| {
                    "Roblox returned 403 without an x-csrf-token header. Your .ROBLOSECURITY cookie may be invalid or expired.".to_string()
                })?;

            self.client
                .post(url)
                .header(COOKIE, &cookie_header)
                .header(REFERER, "https://www.roblox.com/")
                .header(ORIGIN, "https://www.roblox.com")
                .header(CONTENT_TYPE, "application/json")
                .header(CONTENT_LENGTH, "0")
                .header("x-csrf-token", csrf_token)
                .send()
                .await
                .map_err(|e| format!("Network error retrying auth ticket with CSRF token: {e}"))?
        } else {
            first_resp
        };

        let status = final_resp.status();
        if status == StatusCode::UNAUTHORIZED {
            return Err("Session expired (.ROBLOSECURITY rejected by Roblox). Please re-add this account.".to_string());
        }
        if !status.is_success() {
            return Err(format!(
                "Failed to generate authentication ticket (HTTP {status}). Session may be expired or rate-limited."
            ));
        }

        final_resp
            .headers()
            .get("rbx-authentication-ticket")
            .and_then(|v| v.to_str().ok())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .ok_or_else(|| {
                "Roblox succeeded but did not return an rbx-authentication-ticket header.".to_string()
            })
    }

    /// Checks a user's live Roblox presence via POST https://presence.roblox.com/v1/presence/users.
    /// Returns `userPresenceType` (`2` = InGame) and `placeId`.
    pub async fn fetch_user_presence(
        &self,
        user_id: u64,
        cookie: &str,
    ) -> Result<UserPresence, String> {
        let body = serde_json::json!({ "userIds": [user_id] });

        let resp = self
            .client
            .post("https://presence.roblox.com/v1/presence/users")
            .header(COOKIE, format!(".ROBLOSECURITY={cookie}"))
            .header(CONTENT_TYPE, "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Network error contacting Roblox Presence API: {e}"))?;

        let status = resp.status();
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err("Session cookie (.ROBLOSECURITY) rejected by Presence API.".to_string());
        }
        if !status.is_success() {
            return Err(format!("Roblox Presence API returned HTTP {status}."));
        }

        let parsed = resp
            .json::<PresenceApiResponse>()
            .await
            .map_err(|e| format!("Failed to parse Presence API response: {e}"))?;

        parsed
            .user_presences
            .into_iter()
            .next()
            .ok_or_else(|| format!("Presence API returned no entry for user {user_id}."))
    }

    /// Attempts to resolve a modern Roblox share link (`https://www.roblox.com/share?code=XXXX&type=Server`)
    /// into a classic `privateServerLinkCode` via Roblox's sharelinks resolve API or HTTP redirect.
    /// Falls back cleanly to returning the original share `code` if resolution is unavailable.
    pub async fn resolve_share_server_code(
        &self,
        share_code: &str,
        cookie: &str,
    ) -> String {
        let resolve_url = "https://apis.roblox.com/sharelinks/v1/resolve-link";
        let cookie_header = format!(".ROBLOSECURITY={cookie}");
        let payload = serde_json::json!({
            "linkId": share_code,
            "linkType": "Server"
        });

        let first = self
            .client
            .post(resolve_url)
            .header(COOKIE, &cookie_header)
            .header(REFERER, "https://www.roblox.com/")
            .header(ORIGIN, "https://www.roblox.com")
            .header(CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .await;

        let resolved_resp = match first {
            Ok(resp) if resp.status() == StatusCode::FORBIDDEN => {
                if let Some(csrf) = resp
                    .headers()
                    .get("x-csrf-token")
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string)
                {
                    self.client
                        .post(resolve_url)
                        .header(COOKIE, &cookie_header)
                        .header(REFERER, "https://www.roblox.com/")
                        .header(ORIGIN, "https://www.roblox.com")
                        .header(CONTENT_TYPE, "application/json")
                        .header("x-csrf-token", csrf)
                        .json(&payload)
                        .send()
                        .await
                        .ok()
                } else {
                    None
                }
            }
            Ok(resp) => Some(resp),
            Err(_) => None,
        };

        if let Some(resp) = resolved_resp {
            if resp.status().is_success() {
                if let Ok(val) = resp.json::<Value>().await {
                    if let Some(code) = val
                        .get("privateServerInviteData")
                        .and_then(|d| d.get("linkCode"))
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                    {
                        return code.to_string();
                    }
                }
            }
        }

        // Secondary fallback: follow share link redirect to see if Roblox redirects to ?privateServerLinkCode=
        let share_url = format!(
            "https://www.roblox.com/share?code={}&type=Server",
            urlencoding::encode(share_code)
        );
        if let Ok(resp) = self
            .client
            .get(&share_url)
            .header(COOKIE, &cookie_header)
            .send()
            .await
        {
            for (k, v) in resp.url().query_pairs() {
                if k.eq_ignore_ascii_case("privateServerLinkCode") && !v.is_empty() {
                    return v.into_owned();
                }
            }
        }

        share_code.to_string()
    }
}

fn extract_place_id(input: &str) -> Option<u64> {
    if input.chars().all(|c| c.is_ascii_digit()) {
        return input.parse::<u64>().ok().filter(|&id| id > 0);
    }

    // Also support pasted Roblox game URLs like https://www.roblox.com/games/2753915549/Blox-Fruits
    if let Ok(parsed) = url::Url::parse(input) {
        if let Some(host) = parsed.host_str() {
            if host.ends_with("roblox.com") {
                let mut segments = parsed.path_segments()?;
                while let Some(seg) = segments.next() {
                    if seg.eq_ignore_ascii_case("games") {
                        if let Some(id_seg) = segments.next() {
                            if let Ok(id) = id_seg.parse::<u64>() {
                                if id > 0 {
                                    return Some(id);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

fn extract_omni_candidates(root: &Value, limit: usize) -> Vec<OmniCandidate> {
    let mut results = Vec::new();
    let mut seen = HashSet::new();

    let Some(groups) = root.get("searchResults").and_then(Value::as_array) else {
        return results;
    };

    for group in groups {
        let group_type = group
            .get("contentGroupType")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !group_type.is_empty() && !group_type.eq_ignore_ascii_case("Game") {
            continue;
        }

        let Some(contents) = group.get("contents").and_then(Value::as_array) else {
            continue;
        };

        for item in contents {
            if results.len() >= limit {
                return results;
            }
            let universe_id = item.get("universeId").and_then(Value::as_u64).unwrap_or(0);
            if universe_id == 0 || !seen.insert(universe_id) {
                continue;
            }
            let root_place_id = item
                .get("rootPlaceId")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("Untitled Experience")
                .to_string();
            let creator_name = item
                .get("creatorName")
                .and_then(Value::as_str)
                .unwrap_or("Unknown")
                .to_string();
            let playing = item
                .get("playerCount")
                .and_then(Value::as_u64)
                .unwrap_or(0);

            results.push(OmniCandidate {
                universe_id,
                root_place_id,
                name,
                creator_name,
                playing,
            });
        }
    }

    results
}
