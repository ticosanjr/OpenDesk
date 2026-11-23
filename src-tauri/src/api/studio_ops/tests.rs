use super::*;
use crate::config_sync::{
    agents::{self, AgentWriteInput},
    commands::{self, CommandWriteInput},
    content_installation::{parse_markdown, parse_permissions},
    jsonc_editor, markdown,
    skills::{self, SkillWriteInput},
};
use std::io::Write;
use tempfile::TempDir;

fn studio_paths(temp: &TempDir) -> (StudioPaths, StudioPaths) {
    let global = StudioPaths {
        global_dir: temp.path().join("global"),
        workspace_root: None,
    };
    let workspace = StudioPaths {
        global_dir: temp.path().join("global"),
        workspace_root: Some(temp.path().join("ws")),
    };
    (global, workspace)
}

fn agent_input() -> AgentWriteInput {
    AgentWriteInput {
        name: Some("Auditor".into()),
        avatar: Some("🛡️".into()),
        description: Some("Auditor".into()),
        mode: Some("subagent".into()),
        model: None,
        color: Some("#60a5fa".into()),
        router_policy: None,
        hidden: false,
        steps: None,
        disabled: false,
        permissions: Vec::new(),
        system: "Você audita.".into(),
        expected_sha256: None,
    }
}

#[test]
fn copies_agents_and_commands_between_global_and_workspace() {
    let temp = TempDir::new().expect("temp");
    let (global, workspace) = studio_paths(&temp);
    let backups = temp.path().join("backups");
    agents::write(&global, "global", "auditor", agent_input(), &backups).expect("agent");
    let source = StudioItem::new(&global, "global", StudioKind::Agent, "auditor");
    let target = StudioItem::new(&workspace, "workspace", StudioKind::Agent, "auditor-copy");
    source
        .copy_to(&target, false, &backups)
        .expect("copy agent");
    assert_eq!(
        agents::get(&workspace, "workspace", "auditor-copy")
            .expect("copied agent")
            .name
            .as_deref(),
        Some("Auditor")
    );

    commands::write(
        &global,
        "global",
        "revisar",
        CommandWriteInput {
            description: Some("Revisar".into()),
            agent: Some("auditor".into()),
            model: None,
            subagent: None,
            template: "Revise $ARGUMENTS".into(),
            expected_sha256: None,
        },
        &backups,
    )
    .expect("command");
    let source = StudioItem::new(&global, "global", StudioKind::Command, "revisar");
    let target = StudioItem::new(&workspace, "workspace", StudioKind::Command, "revisar");
    source
        .copy_to(&target, false, &backups)
        .expect("copy command");
    assert_eq!(
        commands::get(&workspace, "workspace", "revisar")
            .expect("copied command")
            .template,
        "Revise $ARGUMENTS"
    );
}

#[test]
fn copies_skills_and_mcp_servers() {
    let temp = TempDir::new().expect("temp");
    let (global, workspace) = studio_paths(&temp);
    let backups = temp.path().join("backups");
    skills::write(
        &global,
        "global",
        "pesquisa",
        SkillWriteInput {
            name: Some("Pesquisa".into()),
            description: Some("Pesquisa".into()),
            body: "Use fontes confiáveis.".into(),
            files: vec![],
            expected_sha256: None,
        },
        &backups,
    )
    .expect("skill");
    let source = StudioItem::new(&global, "global", StudioKind::Skill, "pesquisa");
    let target = StudioItem::new(&workspace, "workspace", StudioKind::Skill, "pesquisa");
    source
        .copy_to(&target, false, &backups)
        .expect("copy skill");
    assert_eq!(
        skills::get(&workspace, "workspace", "pesquisa")
            .expect("copied skill")
            .name,
        "Pesquisa"
    );

    let global_config = global.config_path("global").expect("global config");
    jsonc_editor::upsert_mcp_server(
        &global_config,
        "fetch",
        &json!({"type": "local", "command": ["npx", "fetch"]}),
        None,
        &backups,
    )
    .expect("mcp");
    let source = StudioItem::new(&global, "global", StudioKind::Mcp, "fetch");
    let target = StudioItem::new(&workspace, "workspace", StudioKind::Mcp, "fetch");
    source.copy_to(&target, false, &backups).expect("copy mcp");
    let workspace_doc = jsonc_editor::read_for_scope(&workspace, "workspace").expect("config");
    assert!(
        jsonc_editor::list_mcp_servers(&workspace_doc)
            .iter()
            .any(|server| server.name == "fetch")
    );
}

#[test]
fn refusing_to_overwrite_a_copy_keeps_the_target() {
    let temp = TempDir::new().expect("temp");
    let (global, workspace) = studio_paths(&temp);
    let backups = temp.path().join("backups");
    agents::write(&global, "global", "auditor", agent_input(), &backups).expect("agent");
    agents::write(&workspace, "workspace", "auditor", agent_input(), &backups).expect("target");
    let source = StudioItem::new(&global, "global", StudioKind::Agent, "auditor");
    let target = StudioItem::new(&workspace, "workspace", StudioKind::Agent, "auditor");
    assert!(source.copy_to(&target, false, &backups).is_err());
}

#[test]
fn imported_agent_frontmatter_is_parsed() {
    let content = r#"---
name: Auditor
description: Audita
mode: subagent
model: deepseek/deepseek-flash
avatar: "🛡️"
permissions:
  - action: edit
    resource: "*"
    effect: deny
---
Você audita."#;
    let (frontmatter, body) = parse_markdown(content);
    assert_eq!(
        markdown::get_string(&frontmatter, "name").as_deref(),
        Some("Auditor")
    );
    assert_eq!(body.trim(), "Você audita.");
    let permissions = parse_permissions(&frontmatter).expect("permissions");
    assert_eq!(permissions.len(), 1);
    assert_eq!(permissions[0].action, "edit");
    assert_eq!(permissions[0].effect, "deny");
}

#[test]
fn imported_command_uses_body_as_template() {
    let content = r#"---
description: Revisar
agent: auditor
---
Revise $ARGUMENTS"#;
    let (frontmatter, body) = parse_markdown(content);
    assert_eq!(body.trim(), "Revise $ARGUMENTS");
    assert_eq!(
        markdown::get_string(&frontmatter, "agent").as_deref(),
        Some("auditor")
    );
}

#[test]
fn zip_import_groups_skill_assets() {
    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default();
        writer
            .start_file("repo-main/skills/revisor/SKILL.md", options)
            .expect("skill");
        writer
            .write_all(b"---\nname: Revisor\ndescription: Revisa\n---\nUse.\n")
            .expect("write");
        writer
            .start_file("repo-main/skills/revisor/reference.md", options)
            .expect("ref");
        writer.write_all(b"referencia").expect("write ref");
        writer.finish().expect("finish");
    }
    let mut items = Vec::new();
    let mut warnings = Vec::new();
    collect_zip(&buffer.into_inner(), &mut items, &mut warnings).expect("zip");
    let skill = items
        .iter()
        .find(|item| item.kind == "skill")
        .expect("skill item");
    assert_eq!(skill.id, "revisor");
    assert_eq!(skill.files.len(), 1);
    assert_eq!(skill.files[0].path, "reference.md");
    assert_eq!(skill.files[0].content, "referencia");
}
