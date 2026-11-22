//! TLS termination for the local API (D1.2).
//!
//! When `OPENDESK_MTLS_CA`, `OPENDESK_MTLS_CERT` and `OPENDESK_MTLS_KEY` are
//! set, the API listener requires a client certificate signed by the CA. The
//! material is validated at startup and reloaded in the background; existing
//! connections keep the old config until they finish, so a rotation does not
//! drop in-flight jobs.

use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
    mtls,
};
use axum::Router;
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto,
    service::TowerToHyperService,
};
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer},
    server::WebPkiClientVerifier,
};
use rustls_pemfile::{certs, private_key};
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

#[derive(Debug, Clone)]
pub struct TlsEnv {
    pub ca: PathBuf,
    pub cert: PathBuf,
    pub key: PathBuf,
}

impl TlsEnv {
    /// `Ok(None)` means TLS is not configured; a partial configuration is an
    /// error so a mis-typed env var does not silently fall back to HTTP.
    pub fn from_env() -> AppResult<Option<Self>> {
        let ca = std::env::var("OPENDESK_MTLS_CA").ok();
        let cert = std::env::var("OPENDESK_MTLS_CERT").ok();
        let key = std::env::var("OPENDESK_MTLS_KEY").ok();
        match (ca, cert, key) {
            (None, None, None) => Ok(None),
            (Some(ca), Some(cert), Some(key)) => Ok(Some(Self {
                ca: PathBuf::from(ca),
                cert: PathBuf::from(cert),
                key: PathBuf::from(key),
            })),
            _ => Err(AppError::BadRequest(
                "configure OPENDESK_MTLS_CA, OPENDESK_MTLS_CERT e OPENDESK_MTLS_KEY juntos".into(),
            )),
        }
    }
}

pub fn build_server_config(env: &TlsEnv) -> AppResult<Arc<ServerConfig>> {
    // Other crates may enable aws-lc-rs; pin the provider explicitly so the
    // server does not depend on cargo feature unification.
    let _ = rustls::crypto::ring::default_provider().install_default();
    mtls::validate(&env.ca, &env.cert, &env.key)?;
    let chain = load_certs(&env.cert)?;
    let key = load_key(&env.key)?;
    let mut roots = rustls::RootCertStore::empty();
    for cert in load_certs(&env.ca)? {
        roots
            .add(cert)
            .map_err(|error| AppError::BadRequest(format!("CA inválida: {error}")))?;
    }
    let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
        .build()
        .map_err(|error| AppError::BadRequest(format!("client verifier: {error}")))?;
    let config = ServerConfig::builder()
        .with_client_cert_verifier(verifier)
        .with_single_cert(chain, key)
        .map_err(|error| AppError::BadRequest(format!("TLS server config: {error}")))?;
    Ok(Arc::new(config))
}

fn load_certs(path: &Path) -> AppResult<Vec<CertificateDer<'static>>> {
    let file = File::open(path)
        .map_err(|error| AppError::BadRequest(format!("{}: {error}", path.display())))?;
    certs(&mut BufReader::new(file))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::BadRequest(format!("certificado PEM inválido: {error}")))
}

fn load_key(path: &Path) -> AppResult<PrivateKeyDer<'static>> {
    let file = File::open(path)
        .map_err(|error| AppError::BadRequest(format!("{}: {error}", path.display())))?;
    private_key(&mut BufReader::new(file))
        .map_err(|error| AppError::BadRequest(format!("chave PEM inválida: {error}")))?
        .ok_or_else(|| AppError::BadRequest(format!("{} não contém chave privada", path.display())))
}

pub struct Reloader {
    env: TlsEnv,
    current: RwLock<Arc<ServerConfig>>,
    stamps: RwLock<[Option<std::time::SystemTime>; 3]>,
}

impl Reloader {
    pub fn new(env: TlsEnv) -> AppResult<Arc<Self>> {
        let current = build_server_config(&env)?;
        let stamps = stamp(&[&env.ca, &env.cert, &env.key]);
        Ok(Arc::new(Self {
            env,
            current: RwLock::new(current),
            stamps: RwLock::new(stamps),
        }))
    }

    pub fn current(&self) -> Arc<ServerConfig> {
        self.current
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }

    /// Rebuilds the config when any file changed. A failure keeps the previous
    /// material alive and is returned for logging.
    pub fn reload_if_changed(&self) -> AppResult<bool> {
        let next = stamp(&[&self.env.ca, &self.env.cert, &self.env.key]);
        if *self
            .stamps
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
            == next
        {
            return Ok(false);
        }
        let config = build_server_config(&self.env)?;
        *self
            .current
            .write()
            .unwrap_or_else(|poison| poison.into_inner()) = config;
        *self
            .stamps
            .write()
            .unwrap_or_else(|poison| poison.into_inner()) = next;
        Ok(true)
    }
}

fn stamp(paths: &[&PathBuf]) -> [Option<std::time::SystemTime>; 3] {
    [0, 1, 2].map(|index| {
        paths
            .get(index)
            .and_then(|path| std::fs::metadata(path).ok())
            .and_then(|meta| meta.modified().ok())
    })
}

pub async fn spawn_tls_server(
    listener: TcpListener,
    app: Router,
    reloader: Arc<Reloader>,
) -> AppResult<()> {
    let watcher = reloader.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            if let Err(error) = watcher.reload_if_changed() {
                tracing::warn!(error = %error, "TLS material reload failed; keeping previous config");
            }
        }
    });
    loop {
        let (stream, _) = listener
            .accept()
            .await
            .map_err(|error| AppError::Internal(format!("TLS accept: {error}")))?;
        let config = reloader.current();
        let app = app.clone();
        tokio::spawn(async move {
            let acceptor = TlsAcceptor::from(config);
            let tls = match acceptor.accept(stream).await {
                Ok(tls) => tls,
                Err(error) => {
                    tracing::debug!(error = %error, "client TLS handshake rejected");
                    return;
                }
            };
            let service = TowerToHyperService::new(app);
            let builder = auto::Builder::new(TokioExecutor::new());
            if let Err(error) = builder
                .serve_connection_with_upgrades(TokioIo::new(tls), service)
                .await
            {
                tracing::debug!(error = %error, "TLS connection ended");
            }
        });
    }
}

/// Starts TLS when configured; returns `false` when the caller should keep the
/// plain HTTP listener.
pub async fn start_if_configured(
    listener: std::net::TcpListener,
    state: Arc<AppState>,
) -> AppResult<bool> {
    let Some(env) = TlsEnv::from_env()? else {
        return Ok(false);
    };
    let listener = TcpListener::from_std(listener)
        .map_err(|error| AppError::Internal(format!("TLS listener: {error}")))?;
    let reloader = Reloader::new(env)?;
    let app = crate::api::router(state);
    tokio::spawn(async move {
        if let Err(error) = spawn_tls_server(listener, app, reloader).await {
            tracing::error!(error = %error, "OpenDesk TLS API stopped");
        }
    });
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn openssl(args: &[&str]) -> bool {
        Command::new("openssl")
            .args(args)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn material(dir: &Path) -> Option<TlsEnv> {
        if !openssl(&["version"]) {
            return None;
        }
        let ca_key = dir.join("ca.key");
        let ca = dir.join("ca.pem");
        let key = dir.join("node.key");
        let csr = dir.join("node.csr");
        let cert = dir.join("node.pem");
        openssl(&[
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "1",
            "-subj",
            "/CN=CA",
            "-keyout",
            ca_key.to_str()?,
            "-out",
            ca.to_str()?,
        ])
        .then_some(())?;
        openssl(&[
            "req",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-subj",
            "/CN=node",
            "-keyout",
            key.to_str()?,
            "-out",
            csr.to_str()?,
        ])
        .then_some(())?;
        let ext = dir.join("ext.cnf");
        std::fs::write(&ext, "subjectAltName=DNS:node.local\n").ok()?;
        openssl(&[
            "x509",
            "-req",
            "-in",
            csr.to_str()?,
            "-CA",
            ca.to_str()?,
            "-CAkey",
            ca_key.to_str()?,
            "-CAcreateserial",
            "-days",
            "1",
            "-extfile",
            ext.to_str()?,
            "-out",
            cert.to_str()?,
        ])
        .then_some(())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).ok()?;
        }
        Some(TlsEnv { ca, cert, key })
    }

    #[test]
    fn builds_and_reloads_a_client_verifying_config() {
        let dir = tempfile::TempDir::new().expect("temp");
        let Some(env) = material(dir.path()) else {
            return;
        };
        let reloader = Reloader::new(env.clone()).expect("reloader");
        let first = reloader.current();
        assert!(!reloader.reload_if_changed().expect("no change"));
        // Touch the cert with a fresh certificate; the previous Arc must remain
        // valid for in-flight connections while new ones use the new config.
        let second_dir = dir.path().join("second");
        std::fs::create_dir_all(&second_dir).expect("second");
        let second = material(&second_dir);
        if let Some(second) = second {
            std::fs::copy(&second.ca, &env.ca).expect("copy ca");
            std::fs::copy(&second.cert, &env.cert).expect("copy cert");
            std::fs::copy(&second.key, &env.key).expect("copy key");
            assert!(reloader.reload_if_changed().expect("reload"));
            assert!(!Arc::ptr_eq(&first, &reloader.current()));
        }
    }
}
