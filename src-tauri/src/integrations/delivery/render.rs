use super::*;

pub(super) fn attachments_from_paths(paths: &[String]) -> Vec<EmailAttachment> {
    paths
        .iter()
        .take(5)
        .filter_map(|path| {
            let path = PathBuf::from(path);
            let bytes = std::fs::read(&path).ok()?;
            if !smtp_mailer::attachment_limit(bytes.len()) {
                return None;
            }
            Some(EmailAttachment {
                filename: path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("artifact")
                    .to_string(),
                mime: mime_for_path(&path).to_string(),
                bytes,
            })
        })
        .collect()
}

pub(super) fn build_html(
    title: &str,
    run: &runs::TaskRun,
    artifacts: &[artifacts::ArtifactProvenance],
) -> String {
    let summary = run
        .summary_response
        .as_deref()
        .unwrap_or("Sem resumo retornado pelo agente.")
        .chars()
        .take(4000)
        .collect::<String>();
    let rows = artifacts
        .iter()
        .map(|artifact| {
            format!(
                "<tr><td style=\"padding:6px;border-bottom:1px solid #27272a;font-family:monospace\">{}</td>\
                 <td style=\"padding:6px;border-bottom:1px solid #27272a\">{}</td>\
                 <td style=\"padding:6px;border-bottom:1px solid #27272a\">{}</td></tr>",
                encode_text(&artifact.file_path),
                encode_text(&artifact.created_by_agent),
                artifact.file_size_bytes
            )
        })
        .collect::<String>();

    format!(
        "<div style=\"font-family:Inter,Arial,sans-serif;background:#09090b;color:#e4e4e7;padding:24px\">\
         <h2 style=\"margin:0 0 8px\">{title}</h2>\
         <p style=\"color:#a1a1aa;margin:0 0 16px\">Run concluído no OpenDesk. Modelo <code>{}</code> · agente <code>@{}</code> · custo <code>${:.4}</code>.</p>\
         <h3>Resumo</h3><pre style=\"white-space:pre-wrap;background:#18181b;padding:12px;border-radius:8px;color:#d4d4d8\">{}</pre>\
         <h3>Artefatos</h3>\
         <table style=\"width:100%;border-collapse:collapse;font-size:12px\">\
         <thead><tr><th align=\"left\">Arquivo</th><th align=\"left\">Agente</th><th align=\"left\">Bytes</th></tr></thead>\
         <tbody>{rows}</tbody></table>\
         <p style=\"color:#71717a;font-size:11px;margin-top:24px\">Gerado automaticamente pelo OpenDesk.</p>\
         </div>",
        encode_text(&run.model_id),
        encode_text(&run.agent_name),
        run.estimated_cost_usd,
        encode_text(&summary)
    )
}

pub(super) fn build_attachments(
    artifacts: &[artifacts::ArtifactProvenance],
    workspace_root: Option<&Path>,
) -> Vec<EmailAttachment> {
    let root = workspace_root.and_then(|root| std::fs::canonicalize(root).ok());

    artifacts
        .iter()
        .filter(|artifact| artifact.action_type != "DELETED")
        .take(5)
        .filter_map(|artifact| {
            let path = PathBuf::from(&artifact.absolute_path);
            let canonical = std::fs::canonicalize(&path).ok()?;
            if let Some(root) = &root
                && !canonical.starts_with(root)
            {
                return None;
            }
            let bytes = std::fs::read(&path).ok()?;
            if !smtp_mailer::attachment_limit(bytes.len()) {
                return None;
            }
            Some(EmailAttachment {
                filename: path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("artifact")
                    .to_string(),
                mime: mime_for_path(&path).to_string(),
                bytes,
            })
        })
        .collect()
}
