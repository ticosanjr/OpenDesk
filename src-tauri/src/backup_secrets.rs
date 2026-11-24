//! Password-protected portable secrets for workspace backups.

use crate::{
    error::{AppError, AppResult},
    paths::AppPaths,
};
use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit},
};
use argon2::Argon2;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::BTreeMap;

use crate::secrets::SecretStore;

const MAGIC: &[u8; 12] = b"ODS-SECRETS1";
const SALT: usize = 16;
const NONCE: usize = 12;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PortableSecrets {
    #[serde(default)]
    pub secret_store: BTreeMap<String, String>,
    /// Vault entry id -> plaintext secret.
    #[serde(default)]
    pub vault: BTreeMap<String, String>,
}

pub(super) async fn collect(
    pool: &SqlitePool,
    paths: &AppPaths,
    workspace_id: &str,
) -> AppResult<PortableSecrets> {
    let store = SecretStore::new(paths);
    // Only secrets scoped to this workspace. Global integration tokens stay
    // out of a workspace archive.
    let secret_store = store
        .export_plaintext()?
        .into_iter()
        .filter(|(key, _)| key.contains(workspace_id))
        .collect();
    let rows = sqlx::query_as::<_, (String, Vec<u8>, Vec<u8>)>(
        "SELECT id, secret_nonce, secret_ciphertext FROM vault_entries
         WHERE workspace_id = ?",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;
    let mut vault = BTreeMap::new();
    for (id, nonce, ciphertext) in rows {
        if let Ok(plaintext) = crate::secrets::open(&nonce, &ciphertext) {
            vault.insert(id, plaintext);
        }
    }
    Ok(PortableSecrets {
        secret_store,
        vault,
    })
}

pub(super) fn encrypt(value: &PortableSecrets, passphrase: &str) -> AppResult<Vec<u8>> {
    if passphrase.trim().is_empty() {
        return Err(AppError::BadRequest(
            "senha do backup não pode ser vazia".into(),
        ));
    }
    let mut salt = [0u8; SALT];
    let mut nonce = [0u8; NONCE];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), &salt, &mut key)
        .map_err(|error| AppError::Internal(format!("argon2: {error}")))?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let plaintext = serde_json::to_vec(value)
        .map_err(|error| AppError::Internal(format!("secrets json: {error}")))?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_ref())
        .map_err(|_| AppError::Internal("could not encrypt secrets".into()))?;
    let mut output = Vec::with_capacity(MAGIC.len() + SALT + NONCE + ciphertext.len());
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

pub(super) fn decrypt(bytes: &[u8], passphrase: &str) -> AppResult<PortableSecrets> {
    let header = MAGIC.len() + SALT + NONCE;
    if bytes.len() <= header || &bytes[..MAGIC.len()] != MAGIC {
        return Err(AppError::BadRequest(
            "secrets.enc inválido ou de outra versão".into(),
        ));
    }
    let salt = &bytes[MAGIC.len()..MAGIC.len() + SALT];
    let nonce = &bytes[MAGIC.len() + SALT..header];
    let ciphertext = &bytes[header..];
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|error| AppError::Internal(format!("argon2: {error}")))?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| {
            AppError::BadRequest("senha do backup inválida ou secrets corrompidos".into())
        })?;
    serde_json::from_slice(&plaintext)
        .map_err(|error| AppError::BadRequest(format!("secrets.enc inválido: {error}")))
}

pub(super) async fn apply(
    pool: &SqlitePool,
    paths: &AppPaths,
    workspace_id: &str,
    value: &PortableSecrets,
) -> AppResult<()> {
    SecretStore::new(paths).import_plaintext(&value.secret_store)?;
    for (id, plaintext) in &value.vault {
        let sealed = crate::secrets::seal(plaintext)?;
        sqlx::query(
            "UPDATE vault_entries SET secret_ciphertext = ?, secret_nonce = ?
             WHERE id = ? AND workspace_id = ?",
        )
        .bind(&sealed.ciphertext)
        .bind(sealed.nonce.as_slice())
        .bind(id)
        .bind(workspace_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}
