use crate::{
    db::proposals::{self, ConfigProposal, ProposalInput},
    error::{AppError, AppResult},
};
use serde_json::json;
use sqlx::SqlitePool;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::LearningLesson;
use super::capture::truncate;
use super::lessons::get_lesson;

fn skill_slug(lesson: &LearningLesson) -> String {
    let mut hasher = DefaultHasher::new();
    lesson.fingerprint.hash(&mut hasher);
    let hash = format!("{:08x}", hasher.finish());
    format!(
        "learned-{}-{}",
        lesson.category.replace('_', "-"),
        &hash[..8]
    )
}

fn skill_payload(lesson: &LearningLesson) -> serde_json::Value {
    let evidence: serde_json::Value =
        serde_json::from_str(&lesson.evidence_json).unwrap_or_else(|_| json!({}));
    let run = evidence
        .get("run_id")
        .and_then(|value| value.as_str())
        .unwrap_or("desconhecido");
    let agent = lesson.agent_name.as_deref().unwrap_or("router");
    let model = lesson.model_id.as_deref().unwrap_or("modelo padrão");
    let mut steps = vec![
        "Reproduza o cenário do último run citado na evidência.".to_string(),
        format!("Confirme o agente ({agent}) e o modelo ({model}) usados."),
        "Aplique a verificação descrita abaixo antes de reenfileirar a tarefa.".to_string(),
        "Se o padrão não se repetir, descarte esta skill no Studio.".to_string(),
    ];
    if lesson.category == "failure_pattern" {
        steps.insert(
            1,
            "Leia a mensagem de erro e o diagnóstico da execução.".to_string(),
        );
    } else {
        steps.insert(
            1,
            "Reaproveite a estrutura do resumo bem-sucedido como referência.".to_string(),
        );
    }
    let body = format!(
        "## Quando usar\n\n{} (observado {} vez(es)).\n\n{}\n\n## Passos\n\n{}\n\n## Evidência\n\n- Run: `{run}`\n- Categoria: `{}`\n- Fingerprint: `{}`\n",
        lesson.title,
        lesson.occurrences,
        lesson.detail,
        steps
            .iter()
            .enumerate()
            .map(|(index, step)| format!("{}. {step}", index + 1))
            .collect::<Vec<_>>()
            .join("\n"),
        lesson.category,
        lesson.fingerprint
    );
    json!({
        "id": skill_slug(lesson),
        "scope": "workspace",
        "name": lesson.title,
        "description": format!(
            "Lição aprendida a partir de execuções: {}.",
            truncate(&lesson.title, 120)
        ),
        "body": body,
        "files": [],
    })
}

/// Promotes a lesson into the existing Studio proposal flow. The file is only
/// written when a human approves the proposal.
pub async fn promote(
    pool: &SqlitePool,
    workspace_id: &str,
    id: &str,
    requested_by: Option<&str>,
) -> AppResult<ConfigProposal> {
    let lesson = get_lesson(pool, workspace_id, id).await?;
    if let Some(proposal_id) = &lesson.proposal_id {
        return proposals::get(pool, proposal_id).await.map_err(Into::into);
    }
    if lesson.status == "DISMISSED" {
        return Err(AppError::Conflict(
            "lição descartada não pode ser promovida".into(),
        ));
    }
    let payload = skill_payload(&lesson);
    let proposal = proposals::create(
        pool,
        workspace_id,
        ProposalInput {
            kind: "SKILL".into(),
            action: "CREATE".into(),
            title: format!("Skill aprendida: {}", lesson.title),
            description: Some(
                "Gerada pelo loop de aprendizado a partir de execuções reais. Revise antes de aplicar."
                    .into(),
            ),
            payload,
            requested_by: requested_by.map(str::to_string),
            source_run_id: lesson.run_id.clone(),
            source_task_id: lesson.task_id.clone(),
        },
    )
    .await?;
    sqlx::query(
        "UPDATE learning_lessons SET status = 'PROMOTED', proposal_id = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ? AND workspace_id = ?",
    )
    .bind(&proposal.id)
    .bind(id)
    .bind(workspace_id)
    .execute(pool)
    .await?;
    Ok(proposal)
}
