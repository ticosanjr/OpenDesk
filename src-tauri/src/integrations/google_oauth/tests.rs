use super::*;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[test]
fn pkce_challenge_and_url_are_well_formed() {
    let verifier = random_urlsafe(32);
    assert!(verifier.len() >= 43);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    assert_eq!(challenge.len(), 43);
    assert!(!challenge.contains('='));
    assert!(GOOGLE_SCOPES.contains("drive.file"));
    assert!(GOOGLE_SCOPES.contains("gmail.send"));
    assert!(GOOGLE_SCOPES.contains("gmail.readonly"));
    assert!(GOOGLE_SCOPES.contains("tasks"));
}

#[tokio::test]
async fn status_reports_a_disconnected_workspace_without_network() {
    let ctx = crate::test_support::context().await;
    let oauth = GoogleOAuth::new(
        ctx.state.db.clone(),
        Arc::new(crate::secrets::SecretStore::new(&ctx.state.paths)),
        crate::events::EventBus::new(8),
    );
    let status = oauth.status(&ctx.workspace_id).await.expect("status");
    assert!(!status.connected);
    assert!(status.email.is_none());
    assert!(status.callback_url.ends_with("/callback"));

    let error = oauth
        .access_token(&ctx.workspace_id)
        .await
        .expect_err("not connected");
    assert!(format!("{error}").contains("not connected"));
    assert!(oauth.disconnect(&ctx.workspace_id).await.is_ok());
}
