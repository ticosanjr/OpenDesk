use super::*;
use crate::config_sync::{
    agents::AgentWriteInput, commands::CommandWriteInput, skills::SkillWriteInput,
};
use serde_json::{Value, json};
use tempfile::TempDir;

fn paths(temp: &TempDir) -> StudioPaths {
    StudioPaths {
        global_dir: temp.path().join("global"),
        workspace_root: Some(temp.path().join("ws")),
    }
}

fn seed(paths: &StudioPaths, backups: &Path) {
    agents::write(
        paths,
        "workspace",
        "auditor",
        AgentWriteInput {
            name: Some("Auditor".into()),
            avatar: None,
            description: None,
            mode: Some("subagent".into()),
            model: None,
            color: None,
            router_policy: None,
            hidden: false,
            steps: None,
            disabled: false,
            permissions: Vec::new(),
            system: "Você audita.".into(),
            expected_sha256: None,
        },
        backups,
    )
    .expect("agent");
    skills::write(
        paths,
        "workspace",
        "pesquisa",
        SkillWriteInput {
            name: Some("Pesquisa".into()),
            description: None,
            body: "Use fontes.".into(),
            files: Vec::new(),
            expected_sha256: None,
        },
        backups,
    )
    .expect("skill");
    commands::write(
        paths,
        "workspace",
        "revisar",
        CommandWriteInput {
            description: None,
            agent: None,
            model: None,
            subagent: None,
            template: "Revise $ARGUMENTS".into(),
            expected_sha256: None,
        },
        backups,
    )
    .expect("command");
    jsonc_editor::upsert_mcp_server(
        &paths.config_path("workspace").expect("config path"),
        "fetch",
        &json!({ "type": "local", "command": ["npx", "fetch"] }),
        None,
        backups,
    )
    .expect("mcp");
}

#[test]
fn parses_kinds_and_rejects_unknown_ones() {
    assert_eq!(
        StudioKind::parse("agent").expect("agent"),
        StudioKind::Agent
    );
    assert_eq!(
        StudioKind::parse("skill").expect("skill"),
        StudioKind::Skill
    );
    assert_eq!(
        StudioKind::parse("command").expect("command"),
        StudioKind::Command
    );
    assert_eq!(StudioKind::parse("mcp").expect("mcp"), StudioKind::Mcp);
    assert!(StudioKind::parse("plugin").is_err());
}

#[test]
fn resolves_paths_and_lists_all_four_kinds() {
    let temp = TempDir::new().expect("temp");
    let paths = paths(&temp);
    let backups = temp.path().join("backups");
    seed(&paths, &backups);

    for (kind, id) in [
        (StudioKind::Agent, "auditor"),
        (StudioKind::Skill, "pesquisa"),
        (StudioKind::Command, "revisar"),
        (StudioKind::Mcp, "fetch"),
    ] {
        let item = StudioItem::new(&paths, "workspace", kind, id);
        assert!(item.exists().expect("exists"), "{}", kind.as_str());
        assert!(item.path().expect("path").exists(), "{}", kind.as_str());
    }

    for kind in StudioKind::ALL {
        let items = StudioItem::list(&paths, "workspace", kind).expect("list");
        assert_eq!(items.len(), 1, "{}", kind.as_str());
    }
}

#[test]
fn toggles_and_deletes_every_kind() {
    let temp = TempDir::new().expect("temp");
    let paths = paths(&temp);
    let backups = temp.path().join("backups");
    seed(&paths, &backups);

    for (kind, id) in [
        (StudioKind::Agent, "auditor"),
        (StudioKind::Skill, "pesquisa"),
        (StudioKind::Command, "revisar"),
        (StudioKind::Mcp, "fetch"),
    ] {
        let item = StudioItem::new(&paths, "workspace", kind, id);
        item.set_enabled(false, &backups).expect("disable");
        assert!(
            match kind {
                StudioKind::Agent =>
                    agents::get(&paths, "workspace", id)
                        .expect("agent")
                        .disabled,
                StudioKind::Skill =>
                    skills::get(&paths, "workspace", id)
                        .expect("skill")
                        .disabled,
                StudioKind::Command =>
                    commands::get(&paths, "workspace", id)
                        .expect("command")
                        .disabled,
                StudioKind::Mcp => {
                    let document =
                        jsonc_editor::read_for_scope(&paths, "workspace").expect("config");
                    jsonc_editor::list_mcp_servers(&document)
                        .into_iter()
                        .find(|server| server.name == id)
                        .and_then(|server| server.server.get("disabled").and_then(Value::as_bool))
                        .unwrap_or(false)
                }
            },
            "{} disabled",
            kind.as_str()
        );
        item.set_enabled(true, &backups).expect("enable");
        item.delete(&backups).expect("delete");
        assert!(!item.exists().expect("exists"), "{}", kind.as_str());
    }
}
