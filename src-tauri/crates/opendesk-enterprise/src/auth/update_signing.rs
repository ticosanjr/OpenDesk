//! Ed25519 signatures for update manifests.
//!
//! `scripts/release.sh` writes `latest.json`, signs the exact bytes with an
//! offline private key and uploads `latest.json.sig`. The app embeds only the
//! public key and refuses unsigned or tampered manifests, so compromising the
//! GitHub release or its token is not enough to push a malicious binary.

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

/// Public key committed in the repository. Keep this in sync with the private
/// key used by the release machine.
pub const PUBLIC_KEY_B64: &str = include_str!("../update-signing-public-key.txt");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureEnvelope {
    pub algorithm: String,
    pub key_id: String,
    pub signature: String,
}

pub fn public_key_id(public_b64: &str) -> Result<String> {
    let bytes = decode(public_b64.trim())?;
    if bytes.len() != 32 {
        bail!("chave pública ed25519 inválida");
    }
    let digest = Sha256::digest(bytes);
    Ok(digest[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub fn verify_manifest(bytes: &[u8], envelope_json: &str) -> Result<()> {
    verify_with_key(bytes, envelope_json, PUBLIC_KEY_B64)
}

pub fn verify_with_key(bytes: &[u8], envelope_json: &str, public_b64: &str) -> Result<()> {
    let envelope: SignatureEnvelope =
        serde_json::from_str(envelope_json).context("latest.json.sig inválido: JSON malformado")?;
    if envelope.algorithm != "ed25519" {
        bail!(
            "algoritmo de assinatura não suportado: {}",
            envelope.algorithm
        );
    }
    let expected_key = public_key_id(public_b64)?;
    if envelope.key_id != expected_key {
        bail!(
            "assinatura assinada com key_id {}; o app espera {}",
            envelope.key_id,
            expected_key
        );
    }
    let public_bytes = decode(public_b64.trim())?;
    let public_array: [u8; 32] = public_bytes
        .try_into()
        .map_err(|_| anyhow!("chave pública ed25519 inválida"))?;
    let verifying = VerifyingKey::from_bytes(&public_array)
        .map_err(|error| anyhow!("chave pública ed25519 inválida: {error}"))?;
    let signature_bytes = decode(&envelope.signature)?;
    let signature_array: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| anyhow!("assinatura ed25519 inválida"))?;
    let signature = Signature::from_bytes(&signature_array);
    verifying
        .verify(bytes, &signature)
        .map_err(|_| anyhow!("assinatura do latest.json não confere"))
}

pub fn sign_bytes(bytes: &[u8], seed_b64: &str) -> Result<String> {
    let seed = decode(seed_b64.trim())?;
    let seed_array: [u8; 32] = seed
        .try_into()
        .map_err(|_| anyhow!("chave privada ed25519 inválida"))?;
    let signing = SigningKey::from_bytes(&seed_array);
    let signature = signing.sign(bytes);
    let public_b64 = STANDARD.encode(signing.verifying_key().to_bytes());
    let envelope = SignatureEnvelope {
        algorithm: "ed25519".into(),
        key_id: public_key_id(&public_b64)?,
        signature: STANDARD.encode(signature.to_bytes()),
    };
    Ok(format!("{}\n", serde_json::to_string_pretty(&envelope)?))
}

pub fn generate_keypair() -> Result<(String, String)> {
    let mut rng = rand::rngs::OsRng;
    let signing = SigningKey::generate(&mut rng);
    let private_b64 = STANDARD.encode(signing.to_bytes());
    let public_b64 = STANDARD.encode(signing.verifying_key().to_bytes());
    Ok((private_b64, public_b64))
}

fn decode(value: &str) -> Result<Vec<u8>> {
    STANDARD
        .decode(value.trim())
        .map_err(|error| anyhow!("base64 inválido: {error}"))
}

pub fn default_key_path() -> PathBuf {
    if let Ok(path) = std::env::var("OPENDESK_UPDATE_SIGNING_KEY")
        && !path.trim().is_empty()
    {
        return PathBuf::from(path);
    }
    directories::BaseDirs::new()
        .map(|dirs| {
            dirs.config_dir()
                .join("opendesk")
                .join("update-signing.key")
        })
        .unwrap_or_else(|| PathBuf::from("update-signing.key"))
}

fn write_private(path: &Path, value: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(format!("{}\n", value.trim()).as_bytes())?;
    file.sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn warn_if_private_key_is_readable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(path)
            && metadata.permissions().mode() & 0o077 != 0
        {
            eprintln!(
                "aviso: {} pode ser lida por outros usuários; use chmod 600",
                path.display()
            );
        }
    }
}

pub fn run_cli(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("keygen-update") => keygen_cli(&args[1..]),
        Some("sign-update") => sign_cli(&args[1..]),
        Some("verify-update") => verify_cli(&args[1..]),
        _ => bail!("uso: opendesk keygen-update|sign-update|verify-update ..."),
    }
}

fn keygen_cli(args: &[String]) -> Result<()> {
    let mut out = default_key_path();
    let mut public_out: Option<PathBuf> = None;
    let mut force = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--out" => {
                out = PathBuf::from(
                    args.get(index + 1)
                        .ok_or_else(|| anyhow!("--out exige um caminho"))?,
                );
                index += 2;
            }
            "--public-out" => {
                public_out = Some(PathBuf::from(
                    args.get(index + 1)
                        .ok_or_else(|| anyhow!("--public-out exige um caminho"))?,
                ));
                index += 2;
            }
            "--force" => {
                force = true;
                index += 1;
            }
            other => bail!("argumento desconhecido: {other}"),
        }
    }
    if out.exists() && !force {
        bail!(
            "a chave {} já existe; use --force apenas se quiser substituí-la",
            out.display()
        );
    }
    let (private_b64, public_b64) = generate_keypair()?;
    write_private(&out, &private_b64)?;
    if let Some(public_out) = &public_out {
        if let Some(parent) = public_out.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(public_out, format!("{public_b64}\n"))?;
    }
    println!("private: {}", out.display());
    println!("public:  {}", public_b64);
    println!("key_id:  {}", public_key_id(&public_b64)?);
    println!(
        "\nGuarde a chave privada offline e faça backup. A chave pública deve ser\n\
         commitada em crates/opendesk-enterprise/src/update-signing-public-key.txt."
    );
    Ok(())
}

fn sign_cli(args: &[String]) -> Result<()> {
    let manifest = args
        .first()
        .filter(|value| !value.starts_with("--"))
        .map(PathBuf::from)
        .ok_or_else(|| {
            anyhow!("uso: opendesk sign-update <latest.json> <latest.json.sig> [--key <chave>]")
        })?;
    let signature_out = args
        .get(1)
        .filter(|value| !value.starts_with("--"))
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("informe o caminho de saída da assinatura"))?;
    let mut key_path = default_key_path();
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--key" => {
                key_path = PathBuf::from(
                    args.get(index + 1)
                        .ok_or_else(|| anyhow!("--key exige um caminho"))?,
                );
                index += 2;
            }
            other => bail!("argumento desconhecido: {other}"),
        }
    }
    warn_if_private_key_is_readable(&key_path);
    let bytes = fs::read(&manifest)
        .with_context(|| format!("não foi possível ler {}", manifest.display()))?;
    let seed = fs::read_to_string(&key_path)
        .with_context(|| format!("não foi possível ler a chave {}", key_path.display()))?;
    let envelope = sign_bytes(&bytes, &seed)?;
    fs::write(&signature_out, envelope)?;
    println!("assinatura: {}", signature_out.display());
    Ok(())
}

fn verify_cli(args: &[String]) -> Result<()> {
    let manifest = args
        .first()
        .ok_or_else(|| anyhow!("uso: opendesk verify-update <latest.json> <latest.json.sig>"))?;
    let signature = args
        .get(1)
        .ok_or_else(|| anyhow!("uso: opendesk verify-update <latest.json> <latest.json.sig>"))?;
    let bytes = fs::read(manifest).with_context(|| format!("não foi possível ler {}", manifest))?;
    let envelope = fs::read_to_string(signature)
        .with_context(|| format!("não foi possível ler {}", signature))?;
    verify_manifest(&bytes, &envelope)?;
    println!("assinatura ok (key_id {})", public_key_id(PUBLIC_KEY_B64)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> (String, String) {
        let signing = SigningKey::from_bytes(&[7u8; 32]);
        (
            STANDARD.encode(signing.to_bytes()),
            STANDARD.encode(signing.verifying_key().to_bytes()),
        )
    }

    #[test]
    fn signs_and_verifies_the_exact_manifest_bytes() {
        let (private, public) = test_key();
        let bytes = br#"{"version":"1.2.3","assets":[]}"#;
        let envelope = sign_bytes(bytes, &private).expect("sign");
        verify_with_key(bytes, &envelope, &public).expect("verify");
        let tampered = br#"{"version":"9.9.9","assets":[]}"#;
        assert!(verify_with_key(tampered, &envelope, &public).is_err());
    }

    #[test]
    fn rejects_unknown_key_id_and_algorithm() {
        let (private, public) = test_key();
        let bytes = b"manifest";
        let envelope = sign_bytes(bytes, &private).expect("sign");
        let mut value: serde_json::Value = serde_json::from_str(&envelope).expect("json");
        value["key_id"] = serde_json::json!("deadbeef");
        assert!(verify_with_key(bytes, &value.to_string(), &public).is_err());
        value["key_id"] = serde_json::json!(public_key_id(&public).unwrap());
        value["algorithm"] = serde_json::json!("rsa");
        assert!(verify_with_key(bytes, &value.to_string(), &public).is_err());
    }

    #[test]
    fn committed_public_key_is_valid() {
        assert_eq!(decode(PUBLIC_KEY_B64.trim()).expect("base64").len(), 32);
        assert_eq!(public_key_id(PUBLIC_KEY_B64).expect("key id").len(), 12);
    }

    #[test]
    fn generated_keypair_round_trips() {
        let (private, public) = generate_keypair().expect("keygen");
        let bytes = b"release manifest";
        let envelope = sign_bytes(bytes, &private).expect("sign");
        verify_with_key(bytes, &envelope, &public).expect("verify");
    }
}
