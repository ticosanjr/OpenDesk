use crate::{
    app_state::AppState,
    db::workspaces,
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use std::{
    path::{Path as FsPath, PathBuf},
    sync::Arc,
};

#[derive(Debug, serde::Serialize)]
pub struct Blueprint {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub agents: &'static [&'static str],
    pub skills: &'static [&'static str],
}

const BLUEPRINTS: &[Blueprint] = &[
    Blueprint {
        id: "conteudo",
        name: "Equipe de Conteúdo / YouTube",
        description: "Pesquisa, roteiro e edição para vídeos e newsletters.",
        agents: &["pesquisador", "roteirista", "editor"],
        skills: &["roteiro"],
    },
    Blueprint {
        id: "engenharia",
        name: "Equipe de Engenharia de Software",
        description: "Arquitetura, implementação e revisão de código.",
        agents: &["arquiteto", "implementador", "revisor"],
        skills: &["revisao"],
    },
    Blueprint {
        id: "pesquisa",
        name: "Equipe de Pesquisa & Mercado",
        description: "Coleta de fontes, análise e síntese executiva.",
        agents: &["pesquisador-mercado", "analista", "sintetizador"],
        skills: &["pesquisa"],
    },
    Blueprint {
        id: "operacoes",
        name: "Equipe de Operações",
        description: "Rotinas, monitoramento e resposta a incidentes.",
        agents: &["operador", "monitor", "comunicador"],
        skills: &["incidentes"],
    },
];

pub async fn list_blueprints() -> Json<Vec<&'static Blueprint>> {
    Json(BLUEPRINTS.iter().collect())
}

#[derive(Debug, Deserialize, Default)]
pub struct ApplyQuery {
    pub workspace_id: Option<String>,
}

pub async fn apply_blueprint(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<ApplyQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let blueprint = BLUEPRINTS
        .iter()
        .find(|item| item.id == id)
        .ok_or_else(|| AppError::NotFound(format!("blueprint {id}")))?;
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let workspace = workspaces::get(&state.db, &workspace_id).await?;
    let root = FsPath::new(&workspace.root_path);
    let agents_dir = root.join(".opencode").join("agents");
    let skills_dir = root.join(".opencode").join("skills");
    std::fs::create_dir_all(&agents_dir)?;
    std::fs::create_dir_all(&skills_dir)?;

    let mut created = Vec::new();
    let mut skipped = Vec::new();
    for agent in blueprint.agents {
        let path = agents_dir.join(format!("{agent}.md"));
        write_once(&path, &agent_markdown(agent), &mut created, &mut skipped)?;
    }
    for skill in blueprint.skills {
        let path = skills_dir.join(skill).join("SKILL.md");
        write_once(&path, &skill_markdown(skill), &mut created, &mut skipped)?;
    }

    state.refresh_watcher().await?;
    Ok(Json(serde_json::json!({
        "blueprint": blueprint.id,
        "created": created,
        "skipped": skipped,
    })))
}

fn write_once(
    path: &PathBuf,
    content: &str,
    created: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> AppResult<()> {
    if path.exists() {
        skipped.push(path.display().to_string());
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    created.push(path.display().to_string());
    Ok(())
}

fn agent_markdown(agent: &str) -> String {
    let (name, color, description, body) = match agent {
        "pesquisador" => (
            "Pesquisador",
            "#38bdf8",
            "Coleta fontes, valida fatos e entrega um dossiê estruturado.",
            "Pesquise com rigor, cite links e separe fatos de opiniões.",
        ),
        "roteirista" => (
            "Roteirista",
            "#f472b6",
            "Escreve roteiros com gancho, retenção e CTA claro.",
            "Escreva em PT-BR falado, frases curtas e sem clichês de IA.",
        ),
        "editor" => (
            "Editor",
            "#a78bfa",
            "Revisa clareza, ritmo, precisão e consistência do material.",
            "Corte excesso, preserve a voz do autor e aponte riscos factuais.",
        ),
        "arquiteto" => (
            "Arquiteto",
            "#60a5fa",
            "Desenha a solução e os contratos antes da implementação.",
            "Prefira mudanças pequenas, reversíveis e verificáveis.",
        ),
        "implementador" => (
            "Implementador",
            "#34d399",
            "Implementa código limpo, testado e sem dependências desnecessárias.",
            "Rode checks, evite código morto e mantenha o estilo do projeto.",
        ),
        "revisor" => (
            "Revisor",
            "#fbbf24",
            "Revisa diffs procurando regressões, riscos e testes faltantes.",
            "Aponte problemas por arquivo e linha com uma correção objetiva.",
        ),
        "pesquisador-mercado" => (
            "Pesquisador de Mercado",
            "#38bdf8",
            "Mapeia concorrentes, preços e sinais de demanda.",
            "Traga evidências, datas e URLs. Não invente números.",
        ),
        "analista" => (
            "Analista",
            "#c084fc",
            "Transforma dados em hipóteses, riscos e recomendações.",
            "Quantifique quando possível e declare as premissas.",
        ),
        "sintetizador" => (
            "Sintetizador",
            "#f472b6",
            "Produz o resumo executivo com decisões e próximos passos.",
            "Comece pela conclusão e mantenha o texto curto.",
        ),
        "operador" => (
            "Operador",
            "#34d399",
            "Executa rotinas, verifica resultados e registra evidências.",
            "Se algo falhar, pare, diagnostique e proponha a correção.",
        ),
        "monitor" => (
            "Monitor",
            "#fbbf24",
            "Observa filas, erros e limites antes que virem incidentes.",
            "Priorize sinais acionáveis e evite ruído.",
        ),
        "comunicador" => (
            "Comunicador",
            "#38bdf8",
            "Escreve avisos claros para o time e para o usuário final.",
            "Diga o que aconteceu, o impacto e o próximo passo.",
        ),
        _ => (
            "Especialista",
            "#60a5fa",
            "Especialista do blueprint.",
            "Siga as regras do workspace e entregue resultados verificáveis.",
        ),
    };
    format!(
        "---\nname: {name}\ndescription: {description}\nmode: subagent\ncolor: \"{color}\"\n---\n\n{body}\n"
    )
}

fn skill_markdown(skill: &str) -> String {
    let (name, description, body) = match skill {
        "roteiro" => (
            "Roteiro",
            "Escreve roteiros thumbnail-first com retenção e loop.",
            "Estruture em gancho, contexto, desenvolvimento, clímax e CTA. Entregue no output da tarefa.",
        ),
        "revisao" => (
            "Revisão",
            "Revisão de código com foco em regressão e clareza.",
            "Liste achados por severidade, arquivo e linha. Rode os testes relevantes.",
        ),
        "pesquisa" => (
            "Pesquisa",
            "Pesquisa de mercado com fontes verificáveis.",
            "Registre fontes, datas e limitações. Separe evidência de interpretação.",
        ),
        "incidentes" => (
            "Incidentes",
            "Diagnóstico e resposta a falhas operacionais.",
            "Identifique o ponto exato de parada, o impacto e a próxima ação segura.",
        ),
        _ => (
            "Especialidade",
            "Skill do blueprint.",
            "Siga as regras do workspace e registre evidências.",
        ),
    };
    format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_blueprint_has_unique_ids() {
        let mut ids = BLUEPRINTS.iter().map(|item| item.id).collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), BLUEPRINTS.len());
        assert!(BLUEPRINTS.iter().all(|item| !item.agents.is_empty()));
    }

    #[test]
    fn agent_markdown_has_frontmatter() {
        let text = agent_markdown("roteirista");
        assert!(text.starts_with("---\nname:"));
        assert!(text.contains("mode: subagent"));
    }
}
