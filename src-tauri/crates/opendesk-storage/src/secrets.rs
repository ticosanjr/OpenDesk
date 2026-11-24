use crate::{
    error::{AppError, AppResult},
    paths::AppPaths,
};
use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit},
};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, RwLock},
};

/// Local secret store for integrations. Values are encrypted with AES-256-GCM
/// using a machine-derived key and written with mode 0600. Callers may also
/// provide an environment variable named `OPENDESK_<NAMESPACE>_<KEY>` (for
/// example `OPENDESK_SMTP_WS_1`), which is read only when no encrypted file
/// exists and is never persisted or logged. The API is intentionally small so a
/// keyring backend can replace it without changing callers.
#[derive(Clone)]
pub struct SecretStore {
    dir: PathBuf,
}

impl SecretStore {
    pub fn new(paths: &AppPaths) -> Self {
        Self::with_dir(paths.data_dir.join("secrets"))
    }

    pub fn with_dir(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        Self { dir }
    }

    pub fn set(&self, namespace: &str, key: &str, value: &str) -> AppResult<()> {
        let path = self.path(namespace, key);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let sealed = seal(value)?;
        let mut output = Vec::with_capacity(12 + sealed.ciphertext.len());
        output.extend_from_slice(&sealed.nonce);
        output.extend_from_slice(&sealed.ciphertext);
        std::fs::write(&path, STANDARD.encode(output))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }

        Ok(())
    }

    pub fn get(&self, namespace: &str, key: &str) -> AppResult<Option<String>> {
        let path = self.path(namespace, key);
        let encoded = match std::fs::read_to_string(&path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = format!(
                    "OPENDESK_{}_{}",
                    sanitize(namespace).to_uppercase(),
                    sanitize(key).to_uppercase()
                );
                return Ok(std::env::var(name).ok().filter(|value| !value.is_empty()));
            }
            Err(error) => return Err(AppError::Io(error)),
        };

        let bytes = STANDARD
            .decode(encoded.trim())
            .map_err(|error| AppError::Internal(format!("invalid secret file: {error}")))?;
        if bytes.len() < 13 {
            return Err(AppError::Internal("invalid secret payload".into()));
        }

        let (nonce, ciphertext) = bytes.split_at(12);
        open(nonce, ciphertext).map(Some)
    }

    /// Decrypts every stored secret for portable backup. The key is the
    /// sanitized file stem, which is stable across machines.
    pub fn export_plaintext(&self) -> AppResult<BTreeMap<String, String>> {
        let mut values = BTreeMap::new();
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(values),
            Err(error) => return Err(AppError::Io(error)),
        };
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".sec") {
                continue;
            }
            let encoded = std::fs::read_to_string(entry.path())?;
            let bytes = STANDARD
                .decode(encoded.trim())
                .map_err(|error| AppError::Internal(format!("invalid secret file: {error}")))?;
            if bytes.len() < 13 {
                continue;
            }
            let (nonce, ciphertext) = bytes.split_at(12);
            values.insert(
                name.trim_end_matches(".sec").to_string(),
                open(nonce, ciphertext)?,
            );
        }
        Ok(values)
    }

    /// Re-seals portable secrets with this machine's key.
    pub fn import_plaintext(&self, values: &BTreeMap<String, String>) -> AppResult<()> {
        std::fs::create_dir_all(&self.dir)?;
        for (stem, value) in values {
            let sealed = seal(value)?;
            let mut output = Vec::with_capacity(12 + sealed.ciphertext.len());
            output.extend_from_slice(&sealed.nonce);
            output.extend_from_slice(&sealed.ciphertext);
            let path = self.dir.join(format!("{stem}.sec"));
            std::fs::write(&path, STANDARD.encode(output))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            }
        }
        Ok(())
    }

    pub fn delete(&self, namespace: &str, key: &str) -> AppResult<()> {
        let path = self.path(namespace, key);
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(AppError::Io(error)),
        }
    }

    fn path(&self, namespace: &str, key: &str) -> PathBuf {
        let name = format!("{}_{}.sec", sanitize(namespace), sanitize(key));
        self.dir.join(name)
    }
}

/// Ciphertext and nonce produced by [`seal`]. Vault rows persist the two parts
/// in separate columns so the nonce can be rotated independently of the
/// ciphertext storage.
#[derive(Debug, Clone)]
pub struct SealedSecret {
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

/// Encrypts a secret with the same machine-derived AES-256-GCM key used by
/// [`SecretStore`]. Callers must persist the nonce and ciphertext together and
/// must never log either the plaintext or the sealed payload.
pub fn seal(plaintext: &str) -> AppResult<SealedSecret> {
    let mut nonce = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&machine_key()));
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
        .map_err(|_| AppError::Internal("could not encrypt secret".into()))?;
    Ok(SealedSecret { nonce, ciphertext })
}

/// Decrypts a payload produced by [`seal`]. A wrong nonce, tampered ciphertext
/// or payload from another machine is rejected.
pub fn open(nonce: &[u8], ciphertext: &[u8]) -> AppResult<String> {
    if nonce.len() != 12 {
        return Err(AppError::Internal("invalid secret payload".into()));
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&machine_key()));
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| AppError::Internal("could not decrypt secret".into()))?;
    String::from_utf8(plaintext).map_err(|_| AppError::Internal("secret is not valid UTF-8".into()))
}

fn machine_key() -> [u8; 32] {
    let machine = std::fs::read_to_string("/etc/machine-id")
        .or_else(|_| std::fs::read_to_string("/var/lib/dbus/machine-id"))
        .unwrap_or_else(|_| {
            std::env::var("HOSTNAME")
                .or_else(|_| std::env::var("COMPUTERNAME"))
                .unwrap_or_else(|_| "opendesk-local".into())
        });
    let mut hasher = Sha256::new();
    hasher.update(b"opendesk-secret-store-v1");
    hasher.update(machine.trim().as_bytes());
    let digest = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&digest[..32]);
    key
}

/// Thread-safe redactor for secrets resolved during a run. Values shorter
/// than six characters are ignored to avoid corrupting ordinary text.
#[derive(Debug, Clone, Default)]
pub struct SecretScrubber {
    values: Arc<RwLock<Vec<ScrubValue>>>,
}

#[derive(Debug, Clone)]
struct ScrubValue {
    label: String,
    variants: Vec<String>,
}

impl SecretScrubber {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a resolved value. The label is used only in the replacement
    /// marker, never in logs.
    pub fn register(&self, label: &str, value: &str) {
        if value.len() < 6 {
            return;
        }
        let mut variants = vec![value.to_string()];
        let base64 = STANDARD.encode(value.as_bytes());
        if base64.len() >= 6 && base64 != value {
            variants.push(base64);
        }
        let url_safe = URL_SAFE_NO_PAD.encode(value.as_bytes());
        if url_safe.len() >= 6 && url_safe != value && !variants.contains(&url_safe) {
            variants.push(url_safe);
        }
        let encoded = percent_encode(value);
        if encoded != value && !variants.contains(&encoded) {
            variants.push(encoded);
        }
        variants.sort_by_key(|item| std::cmp::Reverse(item.len()));
        variants.dedup();
        let mut values = self
            .values
            .write()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(existing) = values.iter_mut().find(|item| item.label == label) {
            existing.variants.extend(variants);
            existing
                .variants
                .sort_by_key(|item| std::cmp::Reverse(item.len()));
            existing.variants.dedup();
            return;
        }
        values.push(ScrubValue {
            label: label.to_string(),
            variants,
        });
    }

    pub fn scrub(&self, text: &str) -> String {
        let values = self
            .values
            .read()
            .unwrap_or_else(|poison| poison.into_inner());
        if values.is_empty() {
            return text.to_string();
        }
        let mut output = Cow::Borrowed(text);
        for item in values.iter() {
            for variant in &item.variants {
                if output.contains(variant.as_str()) {
                    output = Cow::Owned(
                        output.replace(variant, &format!("[REDACTED:vault:{}]", item.label)),
                    );
                }
            }
        }
        output.into_owned()
    }

    pub fn is_empty(&self) -> bool {
        self.values
            .read()
            .map(|values| values.is_empty())
            .unwrap_or(true)
    }

    pub fn clear(&self) {
        self.values
            .write()
            .unwrap_or_else(|poison| poison.into_inner())
            .clear();
    }
}

fn percent_encode(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            output.push(byte as char);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn round_trips_encrypted_secret_and_removes_it() {
        let temp = TempDir::new().expect("temp");
        let store = SecretStore::with_dir(temp.path().join("secrets"));

        store.set("smtp", "ws-1", "super-secret").expect("set");
        assert_eq!(
            store.get("smtp", "ws-1").expect("get").as_deref(),
            Some("super-secret")
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = store.path("smtp", "ws-1");
            let mode = std::fs::metadata(path)
                .expect("metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        let raw = std::fs::read_to_string(store.path("smtp", "ws-1")).expect("raw");
        assert!(!raw.contains("super-secret"));

        store.delete("smtp", "ws-1").expect("delete");
        assert!(store.get("smtp", "ws-1").expect("get").is_none());
    }

    #[test]
    fn seals_and_opens_vault_values() {
        let sealed = seal("vault-value").expect("seal");
        assert_ne!(sealed.ciphertext, b"vault-value".to_vec());
        assert_eq!(
            open(&sealed.nonce, &sealed.ciphertext).expect("open"),
            "vault-value"
        );
    }

    #[test]
    fn rejects_tampered_or_truncated_vault_payloads() {
        let sealed = seal("vault-value").expect("seal");
        let mut tampered = sealed.ciphertext.clone();
        tampered[0] ^= 0x01;
        assert!(open(&sealed.nonce, &tampered).is_err());
        assert!(open(&[0u8; 11], &sealed.ciphertext).is_err());
    }

    #[test]
    fn scrubs_exact_base64_and_percent_encoded_values() {
        let scrubber = SecretScrubber::new();
        scrubber.register("stripe", "super-secret-value");
        assert!(
            scrubber
                .scrub("token super-secret-value end")
                .contains("[REDACTED:vault:stripe]")
        );
        let base64 = STANDARD.encode("super-secret-value");
        assert!(scrubber.scrub(&base64).contains("[REDACTED:vault:stripe]"));
        let encoded = percent_encode("super-secret-value");
        assert!(scrubber.scrub(&encoded).contains("[REDACTED:vault:stripe]"));
    }

    #[test]
    fn ignores_short_values_and_can_be_cleared() {
        let scrubber = SecretScrubber::new();
        scrubber.register("short", "abc");
        assert_eq!(scrubber.scrub("abc"), "abc");
        assert!(scrubber.is_empty());
        scrubber.register("long", "abcdef");
        assert!(!scrubber.is_empty());
        scrubber.register("long", "ghijkl");
        assert!(
            scrubber
                .scrub("abcdef ghijkl")
                .contains("[REDACTED:vault:long]")
        );
        scrubber.clear();
        assert!(scrubber.is_empty());
    }
}
