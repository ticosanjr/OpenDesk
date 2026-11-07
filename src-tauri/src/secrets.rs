use crate::{
    error::{AppError, AppResult},
    paths::AppPaths,
};
use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

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

        let mut nonce_bytes = [0u8; 12];
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.machine_key()));
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce_bytes), value.as_bytes())
            .map_err(|_| AppError::Internal("could not encrypt secret".into()))?;

        let mut output = Vec::with_capacity(12 + ciphertext.len());
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);
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
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.machine_key()));
        let plaintext = cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| AppError::Internal("could not decrypt secret".into()))?;

        String::from_utf8(plaintext)
            .map(Some)
            .map_err(|_| AppError::Internal("secret is not valid UTF-8".into()))
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

    fn machine_key(&self) -> [u8; 32] {
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
}
