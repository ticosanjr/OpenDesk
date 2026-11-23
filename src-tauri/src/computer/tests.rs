use super::*;

#[test]
fn builds_x11_and_wayland_actions() {
    let x11 = ComputerCapabilities {
        available: true,
        backend: "x11".into(),
        display: Some("x11::0".into()),
        screenshot_tool: Some("import".into()),
        input_tool: Some("xdotool".into()),
        reason: None,
    };
    let (tool, args, summary) = build_action(
        &x11,
        "xdotool",
        &ComputerAction::Click {
            x: 10,
            y: 20,
            button: None,
        },
    )
    .expect("click");
    assert_eq!(tool, "xdotool");
    assert_eq!(args, vec!["mousemove", "10", "20", "click", "1"]);
    assert_eq!(summary["type"], "click");

    let wayland = ComputerCapabilities {
        backend: "wayland".into(),
        ..x11
    };
    let (_, args, _) = build_action(
        &wayland,
        "ydotool",
        &ComputerAction::Type {
            text: "segredo".into(),
        },
    )
    .expect("type");
    assert_eq!(args[0], "type");
    assert!(args.contains(&"segredo".to_string()));
}

#[test]
fn rejects_out_of_range_coordinates() {
    let caps = ComputerCapabilities {
        available: true,
        backend: "x11".into(),
        display: Some("x11::0".into()),
        screenshot_tool: None,
        input_tool: Some("xdotool".into()),
        reason: None,
    };
    assert!(build_action(&caps, "xdotool", &ComputerAction::Move { x: 99_999, y: 0 }).is_err());
}

#[test]
fn allowlist_matches_windows_and_apps_case_insensitively() {
    let allowlist = WindowAllowlist {
        enabled: true,
        windows: vec!["chromium".into()],
        apps: vec!["firefox".into()],
    }
    .normalize();
    assert!(allowlist.allows("Chromium — Notícias", "x"));
    assert!(allowlist.allows("Qualquer", "firefox"));
    assert!(!allowlist.allows("Terminal", "bash"));
    let disabled = WindowAllowlist {
        enabled: false,
        windows: vec![],
        apps: vec![],
    };
    assert!(disabled.allows("Terminal", "bash"));
}

#[tokio::test]
async fn stores_sessions_and_requires_approval() {
    let ctx = crate::test_support::context().await;
    let session = create(&ctx.state.db, &ctx.workspace_id, None, true, false)
        .await
        .expect("session");
    assert_eq!(session.status, "PENDING");
    assert!(require_active(&session).is_err());
    let approved = approve(&ctx.state.db, &session.id).await.expect("approve");
    assert_eq!(approved.status, "ACTIVE");
    let ended = end(&ctx.state.db, &session.id).await.expect("end");
    assert_eq!(ended.status, "ENDED");
    assert_eq!(
        list(&ctx.state.db, &ctx.workspace_id)
            .await
            .expect("list")
            .len(),
        1
    );
}
