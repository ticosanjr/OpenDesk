use super::*;

#[tokio::test]
async fn reports_required_and_optional_onboarding_steps() {
    let ctx = test_support::context().await;
    let (status, body) = call(&ctx, Method::GET, "/api/onboarding", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let steps = body["steps"].as_array().expect("steps");
    assert!(
        steps
            .iter()
            .any(|step| step["id"] == "workspace" && step["status"] == "done"),
        "{body}"
    );
    assert!(
        steps
            .iter()
            .any(|step| step["id"] == "engine" && step["required"] == true),
        "{body}"
    );
    assert!(body["required_total"].as_u64().unwrap_or(0) >= 4);
    assert_eq!(body["complete"], false, "{body}");
}
