//! Local daemon endpoint contract shared by the shell and clients.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEndpoint {
    pub host: String,
    pub port: u16,
    pub token: String,
}

/// Builds an HTTP base URL valid for IPv4, host names and IPv6 literals.
pub fn base_url(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("http://[{host}]:{port}")
    } else {
        format!("http://{host}:{port}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_url_handles_ipv4_and_ipv6() {
        assert_eq!(base_url("127.0.0.1", 8080), "http://127.0.0.1:8080");
        assert_eq!(base_url("::1", 8080), "http://[::1]:8080");
    }
}
