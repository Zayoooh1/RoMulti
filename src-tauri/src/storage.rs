use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use windows::{
    core::w,
    Win32::{
        Foundation::{LocalFree, HLOCAL},
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    },
};

const DPAPI_ENTROPY: &[u8] = b"RoMulti-DPAPI-AccountCookie-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicAccount {
    pub user_id: u64,
    pub username: String,
    pub display_name: String,
    pub avatar_url: String,
    pub added_at: u64,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredAccount {
    user_id: u64,
    username: String,
    display_name: String,
    avatar_url: String,
    encrypted_cookie: String,
    added_at: u64,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AccountsFile {
    accounts: Vec<StoredAccount>,
}

impl From<&StoredAccount> for PublicAccount {
    fn from(acc: &StoredAccount) -> Self {
        Self {
            user_id: acc.user_id,
            username: acc.username.clone(),
            display_name: acc.display_name.clone(),
            avatar_url: acc.avatar_url.clone(),
            added_at: acc.added_at,
            tags: acc.tags.clone(),
        }
    }
}

pub struct AccountStorage {
    file_path: PathBuf,
    lock: Mutex<()>,
}

impl AccountStorage {
    pub fn new() -> Result<Self, String> {
        let appdata = std::env::var("APPDATA")
            .map_err(|_| "Failed to resolve %APPDATA% environment variable.".to_string())?;
        let dir = PathBuf::from(appdata).join("RoMulti");
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create storage directory {}: {e}", dir.display()))?;

        let file_path = dir.join("accounts.json");
        Ok(Self {
            file_path,
            lock: Mutex::new(()),
        })
    }

    pub fn list_public_accounts(&self) -> Result<Vec<PublicAccount>, String> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let data = self.read_file_unlocked()?;
        Ok(data.accounts.iter().map(PublicAccount::from).collect())
    }

    pub fn upsert_account(
        &self,
        user_id: u64,
        username: String,
        display_name: String,
        avatar_url: String,
        cookie: &str,
    ) -> Result<PublicAccount, String> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut data = self.read_file_unlocked()?;

        let encrypted_cookie = encrypt_cookie(cookie)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let preserved_tags = data
            .accounts
            .iter()
            .find(|a| a.user_id == user_id)
            .map(|a| a.tags.clone())
            .unwrap_or_default();

        let entry = StoredAccount {
            user_id,
            username,
            display_name,
            avatar_url,
            encrypted_cookie,
            added_at: now,
            tags: preserved_tags,
        };
        let public = PublicAccount::from(&entry);

        if let Some(existing) = data.accounts.iter_mut().find(|a| a.user_id == user_id) {
            *existing = entry;
        } else {
            data.accounts.push(entry);
        }

        self.write_file_unlocked(&data)?;
        Ok(public)
    }

    pub fn update_account_tags(
        &self,
        user_id: u64,
        tags: Vec<String>,
    ) -> Result<PublicAccount, String> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut data = self.read_file_unlocked()?;

        let existing = data
            .accounts
            .iter_mut()
            .find(|a| a.user_id == user_id)
            .ok_or_else(|| format!("Account {user_id} not found."))?;

        let mut cleaned: Vec<String> = Vec::new();
        for raw in tags {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !cleaned.iter().any(|t| t.eq_ignore_ascii_case(trimmed)) {
                cleaned.push(trimmed.to_string());
            }
        }

        existing.tags = cleaned;
        let public = PublicAccount::from(&*existing);

        self.write_file_unlocked(&data)?;
        Ok(public)
    }

    pub fn update_account_profile(
        &self,
        user_id: u64,
        username: String,
        display_name: String,
        avatar_url: String,
    ) -> Result<PublicAccount, String> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut data = self.read_file_unlocked()?;

        let existing = data
            .accounts
            .iter_mut()
            .find(|a| a.user_id == user_id)
            .ok_or_else(|| format!("Account {user_id} not found."))?;

        existing.username = username;
        existing.display_name = display_name;
        existing.avatar_url = avatar_url;
        let public = PublicAccount::from(&*existing);

        self.write_file_unlocked(&data)?;
        Ok(public)
    }

    pub fn remove_account(&self, user_id: u64) -> Result<(), String> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut data = self.read_file_unlocked()?;
        let before = data.accounts.len();
        data.accounts.retain(|a| a.user_id != user_id);
        if data.accounts.len() == before {
            return Err(format!("Account {user_id} was not found."));
        }
        self.write_file_unlocked(&data)?;
        Ok(())
    }

    pub fn get_account_with_cookie(&self, user_id: u64) -> Result<(PublicAccount, String), String> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let data = self.read_file_unlocked()?;
        let stored = data
            .accounts
            .iter()
            .find(|a| a.user_id == user_id)
            .ok_or_else(|| format!("Account {user_id} not found in storage."))?;

        let decrypted = decrypt_cookie(&stored.encrypted_cookie)?;
        Ok((PublicAccount::from(stored), decrypted))
    }

    fn read_file_unlocked(&self) -> Result<AccountsFile, String> {
        if !self.file_path.exists() {
            return Ok(AccountsFile::default());
        }
        let raw = fs::read_to_string(&self.file_path)
            .map_err(|e| format!("Failed to read accounts.json: {e}"))?;
        if raw.trim().is_empty() {
            return Ok(AccountsFile::default());
        }
        serde_json::from_str::<AccountsFile>(&raw)
            .map_err(|e| format!("Failed to parse accounts.json (file may be corrupted): {e}"))
    }

    fn write_file_unlocked(&self, data: &AccountsFile) -> Result<(), String> {
        let json = serde_json::to_string_pretty(data)
            .map_err(|e| format!("Failed to serialize accounts: {e}"))?;

        let tmp_path = self.file_path.with_extension("json.tmp");
        fs::write(&tmp_path, &json)
            .map_err(|e| format!("Failed to write temporary accounts file: {e}"))?;

        if let Err(rename_err) = fs::rename(&tmp_path, &self.file_path) {
            let _ = fs::remove_file(&tmp_path);
            fs::write(&self.file_path, &json).map_err(|write_err| {
                format!("Failed to save accounts.json (rename: {rename_err}, write: {write_err})")
            })?;
        }

        Ok(())
    }
}

#[cfg(windows)]
fn encrypt_cookie(plaintext: &str) -> Result<String, String> {
    let mut input_bytes = plaintext.as_bytes().to_vec();
    let mut entropy_bytes = DPAPI_ENTROPY.to_vec();

    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: input_bytes.len() as u32,
        pbData: input_bytes.as_mut_ptr(),
    };
    let entropy_blob = CRYPT_INTEGER_BLOB {
        cbData: entropy_bytes.len() as u32,
        pbData: entropy_bytes.as_mut_ptr(),
    };
    let mut out_blob = CRYPT_INTEGER_BLOB::default();

    unsafe {
        CryptProtectData(
            &in_blob,
            w!("RoMulti ROBLOSECURITY"),
            Some(&entropy_blob),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
        .map_err(|e| format!("DPAPI CryptProtectData failed: {e}"))?;

        let slice = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
        let encoded = BASE64.encode(slice);
        let _ = LocalFree(Some(HLOCAL(out_blob.pbData.cast())));
        Ok(encoded)
    }
}

#[cfg(windows)]
fn decrypt_cookie(encoded: &str) -> Result<String, String> {
    let mut encrypted_bytes = BASE64
        .decode(encoded.trim())
        .map_err(|e| format!("Failed to Base64-decode encrypted cookie: {e}"))?;
    let mut entropy_bytes = DPAPI_ENTROPY.to_vec();

    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: encrypted_bytes.len() as u32,
        pbData: encrypted_bytes.as_mut_ptr(),
    };
    let entropy_blob = CRYPT_INTEGER_BLOB {
        cbData: entropy_bytes.len() as u32,
        pbData: entropy_bytes.as_mut_ptr(),
    };
    let mut out_blob = CRYPT_INTEGER_BLOB::default();

    unsafe {
        CryptUnprotectData(
            &in_blob,
            None,
            Some(&entropy_blob),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
        .map_err(|e| {
            format!("DPAPI CryptUnprotectData failed (cookie encrypted under a different Windows user?): {e}")
        })?;

        let slice = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
        let plaintext = String::from_utf8(slice.to_vec())
            .map_err(|e| format!("Decrypted cookie is not valid UTF-8: {e}"));
        let _ = LocalFree(Some(HLOCAL(out_blob.pbData.cast())));
        plaintext
    }
}

#[cfg(not(windows))]
fn encrypt_cookie(_plaintext: &str) -> Result<String, String> {
    Err("Windows DPAPI encryption is only available on Windows.".to_string())
}

#[cfg(not(windows))]
fn decrypt_cookie(_encoded: &str) -> Result<String, String> {
    Err("Windows DPAPI decryption is only available on Windows.".to_string())
}
