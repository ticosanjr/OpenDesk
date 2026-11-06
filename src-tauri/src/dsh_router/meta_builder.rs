use crate::{
    config_sync::{
        agents::{self, AgentWriteInput, PermissionRule},
        commands::{self, CommandWriteInput},
        paths::StudioPaths,
        skills::{self, SkillWriteInput},
    },
    error::{AppError, AppResult},
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct MetaBuildRequest {
    pub prompt: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetaBuildResult {
    pub kind: String,
    pub id: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum MetaIntent {
    Agent {
        id: String,
        model: Option<String>,
        description: String,
        read_only: bool,
    },
    Skill {
        id: String,
        description: String,
    },
    Command {
        id: String,
        description: String,
        template: String,
    },
}

pub fn detect(prompt: &str) -> AppResult<MetaIntent> {
    let lower = prompt.to_lowercase();
    let id = extract_id(prompt).ok_or_else(|| {
        AppError::BadRequest(
            "não encontrei o nome. Use o formato: 'crie um subagente chamado revisor-fiscal…'"
                .into(),
        )
    })?;

    if lower.contains("skill") {
        let description = after_marker(prompt, &["focada em", "focado em", "para"])
            .unwrap_or_else(|| format!("Skill {id} criada pelo Meta-Builder."));
        return Ok(MetaIntent::Skill { id, description });
    }

    if lower.contains("comando") || lower.contains("command") {
        let description = after_marker(prompt, &["para", "que"])
            .unwrap_or_else(|| format!("Comando /{id} criado pelo Meta-Builder."));
        return Ok(MetaIntent::Command {
            id,
            description,
            template: format!("Execute a tarefa: $ARGUMENTS\n\nContexto: {prompt}"),
        });
    }

    if lower.contains("agente") || lower.contains("subagente") {
        let model = extract_model(prompt);
        let description = after_marker(
            prompt,
            &["focado em", "focada em", "especialista em", "para"],
        )
        .unwrap_or_else(|| format!("Agente {id} criado pelo Meta-Builder."));
        let read_only = lower.contains("só pode ler")
            || lower.contains("somente leitura")
            || lower.contains("read-only")
            || lower.contains("read only");
        return Ok(MetaIntent::Agent {
            id,
            model,
            description,
            read_only,
        });
    }

    Err(AppError::BadRequest(
        "pedido não reconhecido. O Meta-Builder cria agentes, subagentes, skills e comandos; use o Studio para MCP e permissões avançadas."
            .into(),
    ))
}

pub fn build(
    paths: &StudioPaths,
    scope: &str,
    request: MetaBuildRequest,
    backups_dir: &Path,
) -> AppResult<MetaBuildResult> {
    let intent = detect(&request.prompt)?;
    match intent {
        MetaIntent::Agent {
            id,
            model,
            description,
            read_only,
        } => {
            let permissions = if read_only {
                vec![
                    PermissionRule {
                        action: "edit".into(),
                        resource: "*".into(),
                        effect: "deny".into(),
                    },
                    PermissionRule {
                        action: "shell".into(),
                        resource: "*".into(),
                        effect: "deny".into(),
                    },
                    PermissionRule {
                        action: "read".into(),
                        resource: "*".into(),
                        effect: "allow".into(),
                    },
                ]
            } else {
                Vec::new()
            };

            let agent = agents::write(
                paths,
                scope,
                &id,
                AgentWriteInput {
                    name: Some(humanize(&id)),
                    avatar: Some("🤖".into()),
                    description: Some(description.clone()),
                    mode: Some("subagent".into()),
                    model,
                    color: Some("#60a5fa".into()),
                    router_policy: None,
                    hidden: false,
                    steps: None,
                    disabled: false,
                    permissions,
                    system: format!(
                        "Você é @{id}. {description}\n\nSiga as instruções do pedido original:\n{}\n",
                        request.prompt.trim()
                    ),
                    expected_sha256: None,
                },
                backups_dir,
            )?;
            Ok(MetaBuildResult {
                kind: "agent".into(),
                id: agent.id,
                path: agent.path.clone(),
                message: format!("Agente @{id} criado em {}", agent.path),
            })
        }
        MetaIntent::Skill { id, description } => {
            let skill = skills::write(
                paths,
                scope,
                &id,
                SkillWriteInput {
                    name: Some(humanize(&id)),
                    description: Some(description.clone()),
                    body: format!(
                        "# Skill {id}\n\n{description}\n\n## Pedido original\n\n{}\n",
                        request.prompt.trim()
                    ),
                    files: Vec::new(),
                    expected_sha256: None,
                },
                backups_dir,
            )?;
            Ok(MetaBuildResult {
                kind: "skill".into(),
                id: skill.id,
                path: skill.path.clone(),
                message: format!("Skill {id} criada em {}", skill.path),
            })
        }
        MetaIntent::Command {
            id,
            description,
            template,
        } => {
            let command = commands::write(
                paths,
                scope,
                &id,
                CommandWriteInput {
                    description: Some(description),
                    agent: None,
                    model: None,
                    subagent: None,
                    template,
                    expected_sha256: None,
                },
                backups_dir,
            )?;
            Ok(MetaBuildResult {
                kind: "command".into(),
                id: command.id,
                path: command.path.clone(),
                message: format!("Comando /{id} criado em {}", command.path),
            })
        }
    }
}

fn extract_id(prompt: &str) -> Option<String> {
    for marker in [
        "chamado ",
        "chamada ",
        "nome ",
        "comando /",
        "agente @",
        "subagente @",
    ] {
        if let Some(index) = prompt.to_lowercase().find(marker) {
            let rest = &prompt[index + marker.len()..];
            let token = rest
                .split_whitespace()
                .next()?
                .trim_matches(|character: char| {
                    !character.is_ascii_alphanumeric()
                        && character != '-'
                        && character != '_'
                        && character != '/'
                });
            if !token.is_empty() {
                return Some(token.to_lowercase());
            }
        }
    }

    prompt
        .split_whitespace()
        .find(|token| token.starts_with('@') || token.starts_with('/'))
        .map(|token| {
            token
                .trim_start_matches(['@', '/'])
                .trim_matches(|character: char| {
                    !character.is_ascii_alphanumeric()
                        && character != '-'
                        && character != '_'
                        && character != '/'
                })
                .to_lowercase()
        })
        .filter(|token| !token.is_empty())
}

fn extract_model(prompt: &str) -> Option<String> {
    let lower = prompt.to_lowercase();
    let index = lower.find("modelo ")?;
    let rest = &prompt[index + "modelo ".len()..];
    let token = rest
        .split_whitespace()
        .next()?
        .trim_matches(|character: char| {
            !character.is_ascii_alphanumeric()
                && character != '-'
                && character != '_'
                && character != '/'
                && character != '.'
                && character != ':'
        });
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

fn humanize(id: &str) -> String {
    id.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn after_marker(prompt: &str, markers: &[&str]) -> Option<String> {
    let lower = prompt.to_lowercase();
    for marker in markers {
        if let Some(index) = lower.find(marker) {
            let value = prompt[index + marker.len()..]
                .trim()
                .trim_end_matches('.')
                .trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_agent_name_model_and_read_only() {
        let intent = detect(
            "Crie um subagente chamado revisor-fiscal focado em contabilidade italiana, usando o modelo deepseek/deepseek-flash, que só pode ler arquivos",
        )
        .expect("intent");
        match intent {
            MetaIntent::Agent {
                id,
                model,
                read_only,
                ..
            } => {
                assert_eq!(id, "revisor-fiscal");
                assert_eq!(model.as_deref(), Some("deepseek/deepseek-flash"));
                assert!(read_only);
            }
            _ => panic!("expected agent"),
        }
    }

    #[test]
    fn detects_skill_and_command() {
        assert!(matches!(
            detect("Crie uma skill chamada relatorio-executivo para relatórios").unwrap(),
            MetaIntent::Skill { .. }
        ));
        assert!(matches!(
            detect("Crie um comando chamado auditar para revisar código").unwrap(),
            MetaIntent::Command { .. }
        ));
    }
}
