//! Lightweight skill evaluation.
//!
//! There is no central skill registry: skills are native files under
//! `.opencode/skills/*/SKILL.md`. The evaluation reads those files and counts
//! mentions in recent run logs, which is enough to spot unused, flaky or
//! frequently used skills without changing the OpenCode contract.

use crate::{error::AppResult, paths::AppPaths};
use serde::Serialize;
use sqlx::SqlitePool;
use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Debug, Clone, Serialize)]
pub struct SkillUsage {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: String,
    pub mentions: usize,
    pub successes: usize,
    pub failures: usize,
    pub success_rate: Option<f64>,
    pub last_used_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone)]
struct RunRow {
    status: String,
    log_path: Option<String>,
    started_at: String,
}

pub async fn evaluate(
    pool: &SqlitePool,
    paths: &AppPaths,
    workspace_root: &Path,
    days: i64,
) -> AppResult<Vec<SkillUsage>> {
    let skill_files = discover(workspace_root);
    let days = days.clamp(1, 365);
    let rows = sqlx::query_as::<_, (String, Option<String>, String)>(
        "SELECT status, log_path, started_at FROM task_runs
         WHERE datetime(started_at) >= datetime('now', ?)
         ORDER BY started_at DESC LIMIT 500",
    )
    .bind(format!("-{days} days"))
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(status, log_path, started_at)| RunRow {
        status,
        log_path,
        started_at,
    })
    .collect::<Vec<_>>();

    let mut usages = Vec::new();
    for (id, path, name, description, updated_at) in skill_files {
        let needles = [
            name.to_lowercase(),
            id.to_lowercase(),
            id.replace('-', " ").to_lowercase(),
        ];
        let mut mentions = 0;
        let mut successes = 0;
        let mut failures = 0;
        let mut last_used_at: Option<String> = None;
        for run in &rows {
            let Some(log_path) = run.log_path.as_deref() else {
                continue;
            };
            let text = read_log(log_path, paths).unwrap_or_default().to_lowercase();
            if text.is_empty() {
                continue;
            }
            if needles
                .iter()
                .any(|needle| !needle.is_empty() && text.contains(needle))
            {
                mentions += 1;
                match run.status.as_str() {
                    "SUCCESS" => successes += 1,
                    "FAILED" | "CANCELLED" => failures += 1,
                    _ => {}
                }
                if last_used_at.is_none() {
                    last_used_at = Some(run.started_at.clone());
                }
            }
        }
        let decided = successes + failures;
        usages.push(SkillUsage {
            id,
            name,
            description,
            path: path.to_string_lossy().to_string(),
            mentions,
            successes,
            failures,
            success_rate: (decided > 0).then(|| successes as f64 / decided as f64),
            last_used_at,
            updated_at,
        });
    }
    usages.sort_by(|left, right| {
        right
            .mentions
            .cmp(&left.mentions)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(usages)
}

fn discover(workspace_root: &Path) -> Vec<(String, PathBuf, String, String, Option<String>)> {
    let root = workspace_root.join(".opencode").join("skills");
    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    let mut skills = Vec::new();
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let file = dir.join("SKILL.md");
        let Ok(content) = std::fs::read_to_string(&file) else {
            continue;
        };
        let id = dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut name = id.clone();
        let mut description = String::new();
        let mut in_frontmatter = false;
        for (index, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if index == 0 && trimmed == "---" {
                in_frontmatter = true;
                continue;
            }
            if in_frontmatter && trimmed == "---" {
                break;
            }
            if let Some(value) = trimmed.strip_prefix("name:") {
                let value = value.trim().trim_matches('"').trim_matches('\'');
                if !value.is_empty() {
                    name = value.to_string();
                }
            } else if let Some(value) = trimmed.strip_prefix("description:") {
                description = value
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
            }
        }
        if description.is_empty() {
            description = content
                .lines()
                .find(|line| {
                    !line.trim().is_empty() && *line != "---" && !line.trim_start().starts_with('#')
                })
                .map(|line| line.trim().chars().take(160).collect())
                .unwrap_or_default();
        }
        let updated_at = std::fs::metadata(&file)
            .and_then(|meta| meta.modified())
            .ok()
            .map(iso_time);
        skills.push((id, file, name, description, updated_at));
    }
    skills
}

fn iso_time(time: SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Utc> = time.into();
    datetime.format("%Y-%m-%dT%H:%M:%fZ").to_string()
}

fn read_log(log_path: &str, paths: &AppPaths) -> Option<String> {
    let path = Path::new(log_path);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        paths.run_logs_dir.join(path)
    };
    let metadata = std::fs::metadata(&path).ok()?;
    if metadata.len() > 2 * 1024 * 1024 {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn discovers_skills_and_reports_mentions() {
        let ctx = crate::test_support::context().await;
        let skill_dir = ctx.workspace_root.join(".opencode/skills/invoice-audit");
        std::fs::create_dir_all(&skill_dir).expect("dir");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: invoice-audit\ndescription: Confere notas fiscais\n---\n\n# Skill\n",
        )
        .expect("skill");

        let log = ctx.state.paths.run_logs_dir.join("run-1.jsonl");
        std::fs::write(&log, "{\"content\":\"use invoice-audit para conferir\"}\n").expect("log");
        sqlx::query(
            "INSERT INTO task_runs
                (id, workspace_id, agent_name, execution_mode, model_id, effort_level, status, log_path, started_at)
             VALUES ('run-1', ?, 'build', 'build', 'test/model', 'medium', 'SUCCESS', ?,
                     strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        )
        .bind(&ctx.workspace_id)
        .bind(log.to_string_lossy().to_string())
        .execute(&ctx.state.db)
        .await
        .expect("run");

        let usage = evaluate(&ctx.state.db, &ctx.state.paths, &ctx.workspace_root, 30)
            .await
            .expect("eval");
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].name, "invoice-audit");
        assert_eq!(usage[0].mentions, 1);
        assert_eq!(usage[0].success_rate, Some(1.0));
    }
}
