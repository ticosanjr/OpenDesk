//! First-class integration with OpenCode Browse (MCP server + Chromium
//! extension). Tasks that need internet navigation get the MCP server and a
//! native skill, without OpenDesk reimplementing browser execution.

use crate::{
    config_sync::{jsonc_editor, paths::StudioPaths, skills, skills::SkillWriteInput},
    db::workspaces,
    error::{AppError, AppResult},
    paths::AppPaths,
};
use serde::Serialize;
use serde_json::json;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

pub const BROWSE_SKILL: &str = "browse-web";
use crate::browse_tags::is_high_risk;
pub use crate::browse_tags::{allowlist_from_tags, needs_browser};

#[derive(Debug, Clone, Serialize)]
pub struct BrowseStatus {
    pub binary: Option<String>,
    pub configured: bool,
    pub skill: bool,
    pub scope: String,
    pub config_path: Option<String>,
    pub install_hint: String,
    pub command: Vec<String>,
}

fn executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn find_binary(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&paths) {
        let candidate = directory.join(name);
        if executable(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let candidate = directory.join(format!("{name}.exe"));
            if executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn server_entry(binary: Option<&Path>) -> serde_json::Value {
    let command = match binary {
        Some(binary) => vec![binary.to_string_lossy().to_string(), "mcp".to_string()],
        None => vec![
            "npx".to_string(),
            "-y".to_string(),
            "@opencode/browse".to_string(),
            "mcp".to_string(),
        ],
    };
    json!({
        "type": "local",
        "command": command,
        "disabled": false
    })
}

fn configured(document: &jsonc_editor::ConfigDocument) -> bool {
    jsonc_editor::list_mcp_servers(document)
        .iter()
        .any(|server| server.name == "browse")
}

fn skill_exists(paths: &StudioPaths, scope: &str) -> bool {
    paths
        .skills_dir(scope)
        .ok()
        .map(|dir| dir.join(BROWSE_SKILL).join("SKILL.md").is_file())
        .unwrap_or(false)
}

fn skill_body() -> String {
    r#"# Navegação web com Browse

Use esta skill quando a tarefa exigir abrir sites, interagir com páginas, fazer login ou coletar conteúdo que depende de JavaScript.

## Servidor
O OpenCode expõe o servidor MCP `browse` (OpenCode Browse). Ele controla o navegador real do usuário, com as abas, logins e extensões já existentes.

## Como usar
1. Verifique `status` antes de começar.
2. Crie uma sessão nomeada com `session_new` e use-a em todas as chamadas.
3. Prefira `execute` para navegar, clicar, preencher e extrair conteúdo.
4. Use `session_allowlist` para limitar a sessão aos domínios da tarefa quando possível.
5. Salve screenshots, HARs e textos extraídos no diretório de saída.

## Regras
- Não use curl para páginas que dependem de JavaScript, login ou interação.
- Em login, 2FA, pagamento ou captcha, use o handoff do Browse e peça a ação ao usuário; não tente contornar.
- Nunca leia nem escreva cookies fora do escopo da sessão.
- Não feche abas do usuário; use apenas abas abertas pelo agente ou emprestadas.
- Se a página bloquear automação, pare e reporte; não tente burlar proteções.
- Ao terminar, feche a sessão e informe as URLs e evidências coletadas.
"#
    .to_string()
}

pub async fn status(
    pool: &SqlitePool,
    _app_paths: &AppPaths,
    workspace_id: &str,
    scope: &str,
) -> AppResult<BrowseStatus> {
    let workspace = workspaces::get(pool, workspace_id).await?;
    let paths = StudioPaths::new(Some(PathBuf::from(&workspace.root_path)));
    let binary = find_binary("browse");
    let document = jsonc_editor::read_for_scope(&paths, scope)?;
    let config_path = paths.config_path(scope).ok();
    let command = server_entry(binary.as_deref())["command"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(BrowseStatus {
        binary: binary.map(|path| path.to_string_lossy().to_string()),
        configured: configured(&document),
        skill: skill_exists(&paths, scope),
        scope: scope.to_string(),
        config_path: config_path.map(|path| path.to_string_lossy().to_string()),
        install_hint: "Instale a extensão Chrome do Browse e rode `npx @opencode/browse install`, ou use o comando npx direto pelo OpenDesk.".into(),
        command,
    })
}

pub async fn enable(
    pool: &SqlitePool,
    app_paths: &AppPaths,
    workspace_id: &str,
    scope: &str,
) -> AppResult<BrowseStatus> {
    let workspace = workspaces::get(pool, workspace_id).await?;
    let paths = StudioPaths::new(Some(PathBuf::from(&workspace.root_path)));
    paths.ensure_scope_layout(scope)?;
    let binary = find_binary("browse");
    let config_path = paths.config_path(scope)?;
    jsonc_editor::upsert_mcp_server(
        &config_path,
        "browse",
        &server_entry(binary.as_deref()),
        None,
        &app_paths.backups_dir,
    )?;
    if !skill_exists(&paths, scope) {
        skills::write(
            &paths,
            scope,
            BROWSE_SKILL,
            SkillWriteInput {
                name: Some("Navegação web com Browse".into()),
                description: Some(
                    "Abre e opera o navegador real via OpenCode Browse para tarefas que precisam de internet."
                        .into(),
                ),
                body: skill_body(),
                files: Vec::new(),
                expected_sha256: None,
            },
            &app_paths.backups_dir,
        )?;
    }
    status(pool, app_paths, workspace_id, scope).await
}

pub async fn disable(
    pool: &SqlitePool,
    app_paths: &AppPaths,
    workspace_id: &str,
    scope: &str,
) -> AppResult<BrowseStatus> {
    let workspace = workspaces::get(pool, workspace_id).await?;
    let paths = StudioPaths::new(Some(PathBuf::from(&workspace.root_path)));
    let config_path = paths.config_path(scope)?;
    if config_path.exists() {
        jsonc_editor::delete_mcp_server(&config_path, "browse", None, &app_paths.backups_dir)?;
    }
    status(pool, app_paths, workspace_id, scope).await
}

/// Best-effort provisioning used by dispatch. It never blocks a task when the
/// browse binary/config cannot be prepared.
pub async fn ensure_for_task(
    pool: &SqlitePool,
    app_paths: &AppPaths,
    workspace_id: &str,
    title: &str,
    description: &str,
    tags: &[String],
) -> AppResult<bool> {
    if !needs_browser(title, description, tags) {
        return Ok(false);
    }
    let allowlist = allowlist_from_tags(tags);
    if is_high_risk(tags) && allowlist.is_empty() {
        return Err(AppError::BadRequest(
            "tarefa de navegador de alto risco sem allowlist; adicione a tag browse-allowlist:dominio1,dominio2"
                .into(),
        ));
    }
    let current = status(pool, app_paths, workspace_id, "workspace").await?;
    if (!current.configured || !current.skill)
        && let Err(error) = enable(pool, app_paths, workspace_id, "workspace").await
    {
        tracing::warn!(
            workspace_id,
            error = %error,
            "browse provisioning failed; task will still include browser instructions"
        );
    }
    Ok(true)
}

#[allow(dead_code)]
pub fn instructions(tags: &[String]) -> String {
    let allowlist = allowlist_from_tags(tags);
    let allow_line = if allowlist.is_empty() {
        "- Nenhuma allowlist configurada; prefira sites públicos e pare em login, 2FA ou pagamento.\n".to_string()
    } else {
        format!(
            "- Use `session_allowlist` com exatamente estes domínios: {}.\n",
            allowlist.join(", ")
        )
    };
    format!(
        "## Navegação web (OpenCode Browse)\n\n\
Esta tarefa precisa navegar na internet. Use o servidor MCP `browse`, que controla o navegador real do usuário.\n\n\
- Comece por `status` e crie uma sessão com `session_new`; reutilize a sessão em todas as chamadas.\n\
- Use `execute` para navegar, clicar, preencher e extrair conteúdo.\n\
{allow_line}\
- Em login, 2FA, pagamento ou captcha, use o handoff e peça a ação ao usuário; não tente contornar.\n\
- Não use curl para páginas que dependem de JavaScript, login ou interação.\n\
- Salve screenshots, HARs e evidências no diretório de saída.\n\
- Ao terminar, feche a sessão e informe as URLs visitadas.\n\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[test]
    fn detects_browser_tasks_conservatively() {
        assert!(needs_browser(
            "Pesquisar preços",
            "Abra https://example.com",
            &[]
        ));
        assert!(needs_browser("Navegar", "navegue na internet", &[]));
        assert!(needs_browser(
            "Tarefa",
            "x",
            &[crate::browse_tags::BROWSE_TAG.into()]
        ));
        assert!(!needs_browser("Corrigir bug", "Ajuste o parser Rust", &[]));
    }

    #[tokio::test]
    async fn task_provisioning_activates_only_for_web_tasks() {
        let ctx = test_support::context().await;
        assert!(
            !ensure_for_task(
                &ctx.db,
                &ctx.paths,
                &ctx.workspace_id,
                "Corrigir parser",
                "Ajuste o Rust",
                &[],
            )
            .await
            .expect("plain task")
        );
        assert!(
            ensure_for_task(
                &ctx.db,
                &ctx.paths,
                &ctx.workspace_id,
                "Pesquisar",
                "Abra https://example.com",
                &[],
            )
            .await
            .expect("web task")
        );
        assert!(
            ctx.workspace_root
                .join(".opencode/skills/browse-web/SKILL.md")
                .is_file()
        );
        let config =
            std::fs::read_to_string(ctx.workspace_root.join("opencode.jsonc")).expect("config");
        assert!(config.contains("\"browse\""));
    }

    #[test]
    fn parses_browse_allowlist_tags() {
        let tags = vec![
            "needs-browser".to_string(),
            "browse-allowlist:https://Example.com/path, docs.example.com".to_string(),
            "browse-allowlist:example.com, *.invalid".to_string(),
        ];
        assert_eq!(
            allowlist_from_tags(&tags),
            vec!["example.com".to_string(), "docs.example.com".to_string()]
        );
        assert!(allowlist_from_tags(&["needs-browser".into()]).is_empty());
    }

    #[test]
    fn instructions_include_the_allowlist() {
        let prompt = instructions(&[
            "needs-browser".into(),
            "browse-allowlist:example.com,docs.example.com".into(),
        ]);
        assert!(prompt.contains("session_allowlist"));
        assert!(prompt.contains("example.com, docs.example.com"));
    }

    #[tokio::test]
    async fn high_risk_browse_task_requires_allowlist() {
        let ctx = test_support::context().await;
        let error = ensure_for_task(
            &ctx.db,
            &ctx.paths,
            &ctx.workspace_id,
            "Pagar boleto",
            "Entre no banco e pague",
            &["needs-browser".into(), "browse-payment".into()],
        )
        .await
        .expect_err("high risk");
        assert!(error.to_string().contains("allowlist"));

        assert!(
            ensure_for_task(
                &ctx.db,
                &ctx.paths,
                &ctx.workspace_id,
                "Pagar boleto",
                "Entre no banco e pague",
                &[
                    "needs-browser".into(),
                    "browse-payment".into(),
                    "browse-allowlist:banco.example.com".into(),
                ],
            )
            .await
            .expect("allowlisted high risk")
        );
    }

    #[tokio::test]
    async fn enable_writes_mcp_entry_and_skill() {
        let ctx = test_support::context().await;
        let status = enable(&ctx.db, &ctx.paths, &ctx.workspace_id, "workspace")
            .await
            .expect("enable");
        assert!(status.configured);
        assert!(status.skill);
        let config =
            std::fs::read_to_string(ctx.workspace_root.join("opencode.jsonc")).expect("config");
        assert!(config.contains("\"browse\""));
        assert!(config.contains("@opencode/browse") || config.contains("browse"));
        let skill = ctx
            .workspace_root
            .join(format!(".opencode/skills/{BROWSE_SKILL}/SKILL.md"));
        assert!(skill.is_file());

        let disabled = disable(&ctx.db, &ctx.paths, &ctx.workspace_id, "workspace")
            .await
            .expect("disable");
        assert!(!disabled.configured);
    }
}
