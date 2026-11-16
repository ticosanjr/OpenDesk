//! mTLS material validation.
//!
//! The transport is enabled later; this module validates the CA, node
//! certificate, private-key permissions, SAN and expiry so a broken enrollment
//! fails before the node starts.

use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::{path::Path, process::Command};

#[derive(Debug, Clone, Serialize)]
pub struct MtlsStatus {
    pub configured: bool,
    pub valid: bool,
    pub ca: Option<String>,
    pub cert: Option<String>,
    pub key: Option<String>,
    pub detail: Option<String>,
}

pub fn from_env() -> MtlsStatus {
    let ca = std::env::var("OPENDESK_MTLS_CA").ok();
    let cert = std::env::var("OPENDESK_MTLS_CERT").ok();
    let key = std::env::var("OPENDESK_MTLS_KEY").ok();
    match (ca, cert, key) {
        (Some(ca), Some(cert), Some(key)) => {
            validate(Path::new(&ca), Path::new(&cert), Path::new(&key)).unwrap_or_else(|error| {
                MtlsStatus {
                    configured: true,
                    valid: false,
                    ca: Some(ca),
                    cert: Some(cert),
                    key: Some(key),
                    detail: Some(format!("{error:#}")),
                }
            })
        }
        _ => MtlsStatus {
            configured: false,
            valid: false,
            ca: None,
            cert: None,
            key: None,
            detail: Some("OPENDESK_MTLS_CA/CERT/KEY não configurados".into()),
        },
    }
}

pub fn validate(ca: &Path, cert: &Path, key: &Path) -> AppResult<MtlsStatus> {
    for path in [ca, cert, key] {
        if !path.is_file() {
            return Err(AppError::BadRequest(format!(
                "arquivo mTLS ausente: {}",
                path.display()
            )));
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(key)?.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(AppError::BadRequest(format!(
                "chave mTLS {} deve ter permissão 0600 (atual {:o})",
                key.display(),
                mode
            )));
        }
    }
    let verify = Command::new("openssl")
        .args(["verify", "-CAfile"])
        .arg(ca)
        .arg(cert)
        .output()
        .map_err(|error| AppError::Internal(format!("openssl indisponível: {error}")))?;
    if !verify.status.success() {
        return Err(AppError::BadRequest(format!(
            "certificado não valida contra a CA: {}",
            String::from_utf8_lossy(&verify.stderr).trim()
        )));
    }
    let expiry = Command::new("openssl")
        .args(["x509", "-checkend", "0", "-noout", "-in"])
        .arg(cert)
        .output()
        .map_err(|error| AppError::Internal(format!("openssl indisponível: {error}")))?;
    if !expiry.status.success() {
        return Err(AppError::BadRequest("certificado mTLS expirado".into()));
    }
    let san = Command::new("openssl")
        .args(["x509", "-noout", "-ext", "subjectAltName", "-in"])
        .arg(cert)
        .output()
        .map_err(|error| AppError::Internal(format!("openssl indisponível: {error}")))?;
    let san_text =
        String::from_utf8_lossy(&san.stdout).to_string() + &String::from_utf8_lossy(&san.stderr);
    if !san_text.contains("DNS:") && !san_text.contains("IP Address:") {
        return Err(AppError::BadRequest(
            "certificado mTLS sem SAN DNS/IP".into(),
        ));
    }
    Ok(MtlsStatus {
        configured: true,
        valid: true,
        ca: Some(ca.to_string_lossy().to_string()),
        cert: Some(cert.to_string_lossy().to_string()),
        key: Some(key.to_string_lossy().to_string()),
        detail: Some("CA, certificado, SAN e validade ok".into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn openssl(args: &[&str]) -> bool {
        Command::new("openssl")
            .args(args)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn validates_generated_ca_and_node_certificate() {
        if !openssl(&["version"]) {
            return;
        }
        let dir = tempfile::tempdir().expect("temp");
        let ca_key = dir.path().join("ca.key");
        let ca = dir.path().join("ca.pem");
        let key = dir.path().join("node.key");
        let csr = dir.path().join("node.csr");
        let cert = dir.path().join("node.pem");
        assert!(
            Command::new("openssl")
                .args([
                    "req",
                    "-x509",
                    "-newkey",
                    "rsa:2048",
                    "-nodes",
                    "-days",
                    "1",
                    "-subj",
                    "/CN=OpenDesk CA",
                    "-keyout"
                ])
                .arg(&ca_key)
                .arg("-out")
                .arg(&ca)
                .output()
                .expect("ca")
                .status
                .success()
        );
        assert!(
            Command::new("openssl")
                .args([
                    "req",
                    "-newkey",
                    "rsa:2048",
                    "-nodes",
                    "-subj",
                    "/CN=worker",
                    "-keyout"
                ])
                .arg(&key)
                .arg("-out")
                .arg(&csr)
                .output()
                .expect("csr")
                .status
                .success()
        );
        let ext = dir.path().join("ext.cnf");
        std::fs::write(&ext, "subjectAltName=DNS:worker.local\n").expect("ext");
        assert!(
            Command::new("openssl")
                .args(["x509", "-req", "-in"])
                .arg(&csr)
                .arg("-CA")
                .arg(&ca)
                .arg("-CAkey")
                .arg(&ca_key)
                .args(["-CAcreateserial", "-days", "1", "-extfile"])
                .arg(&ext)
                .arg("-out")
                .arg(&cert)
                .output()
                .expect("sign")
                .status
                .success()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).expect("mode");
        }
        let status = validate(&ca, &cert, &key).expect("valid");
        assert!(status.valid);
    }
}
