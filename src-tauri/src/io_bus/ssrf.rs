//! Shared SSRF validation for outbound HTTP edges.

use crate::error::{AppError, AppResult};
use reqwest::Url;
use std::net::IpAddr;

/// Validates an outbound URL. Private, loopback, link-local and metadata
/// addresses are rejected unless the caller explicitly opts in. Metadata
/// endpoints stay blocked even with the opt-in.
pub async fn validate_target(raw: &str, allow_private: bool) -> AppResult<Url> {
    let url =
        Url::parse(raw).map_err(|error| AppError::BadRequest(format!("URL inválida: {error}")))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::BadRequest(
            "destino aceita apenas http/https".into(),
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| AppError::BadRequest("URL sem host".into()))?;
    if host == "169.254.169.254" || host == "[fd00:ec2::254]" {
        return Err(AppError::BadRequest(
            "metadata cloud não é um destino permitido".into(),
        ));
    }
    if allow_private {
        return Ok(url);
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        ensure_public(ip)?;
        return Ok(url);
    }
    let port = url.port_or_known_default().unwrap_or(443);
    let mut resolved = false;
    for address in tokio::net::lookup_host((host, port))
        .await
        .map_err(|error| AppError::BadRequest(format!("DNS falhou para {host}: {error}")))?
    {
        resolved = true;
        ensure_public(address.ip())?;
    }
    if !resolved {
        return Err(AppError::BadRequest(format!("host '{host}' não resolveu")));
    }
    Ok(url)
}

fn ensure_public(ip: IpAddr) -> AppResult<()> {
    let blocked = match ip {
        IpAddr::V4(ip) => {
            ip.is_loopback()
                || ip.is_private()
                || ip.is_link_local()
                || ip.is_unspecified()
                || ip.is_broadcast()
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || (ip.segments()[0] & 0xfe00) == 0xfc00
                || (ip.segments()[0] & 0xffc0) == 0xfe80
        }
    };
    if blocked {
        return Err(AppError::BadRequest(format!(
            "destino {ip} não permitido (rede privada/loopback); use allow_private_network=true"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_private_and_metadata_targets() {
        assert!(
            validate_target("http://127.0.0.1:8080", false)
                .await
                .is_err()
        );
        assert!(
            validate_target("http://169.254.169.254/latest/meta-data", true)
                .await
                .is_err()
        );
        assert!(validate_target("http://127.0.0.1:8080", true).await.is_ok());
    }
}
