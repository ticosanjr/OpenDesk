pub use super::models::{CreateWorkspaceInput, UpdateWorkspaceInput, Workspace};
use crate::{
    error::{AppError, AppResult},
    paths::AppPaths,
};
use sqlx::SqlitePool;
use uuid::Uuid;

const SELECT_COLUMNS: &str = "id, name, root_path, description, color_hex, \
    default_router_policy, default_email_recipient, google_drive_folder_id, timezone, \
    is_active, created_at, last_opened_at";

const ALLOWED_POLICIES: [&str; 5] = [
    "COST_BENEFIT",
    "CHEAPEST",
    "BEST_QUALITY",
    "MANUAL",
    "CUSTOM_RULES",
];

fn validate_policy(policy: &str) -> AppResult<String> {
    let normalized = policy.trim().to_uppercase();
    if ALLOWED_POLICIES.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(AppError::BadRequest(format!(
            "invalid router policy '{policy}'"
        )))
    }
}

fn validate_color(color: &str) -> AppResult<String> {
    let value = color.trim();
    if value.len() == 7
        && value.starts_with('#')
        && value[1..].chars().all(|c| c.is_ascii_hexdigit())
    {
        Ok(value.to_lowercase())
    } else {
        Err(AppError::BadRequest(format!(
            "invalid color '{color}'; expected #rrggbb"
        )))
    }
}

fn canonicalize_workspace_path(path: &str, paths: &AppPaths) -> AppResult<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("workspace path is required".into()));
    }
    let candidate = std::path::Path::new(trimmed);
    std::fs::create_dir_all(candidate).map_err(AppError::Io)?;
    let canonical = std::fs::canonicalize(candidate).map_err(AppError::Io)?;
    if !canonical.is_dir() {
        return Err(AppError::BadRequest(format!(
            "workspace path is not a directory: {}",
            canonical.display()
        )));
    }
    paths
        .ensure_workspace_layout(&canonical)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(canonical.to_string_lossy().to_string())
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Workspace>> {
    let query = format!(
        "SELECT {SELECT_COLUMNS} FROM workspaces ORDER BY last_opened_at DESC, created_at DESC"
    );
    Ok(sqlx::query_as::<_, Workspace>(&query)
        .fetch_all(pool)
        .await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Workspace> {
    let query = format!("SELECT {SELECT_COLUMNS} FROM workspaces WHERE id = ?");
    sqlx::query_as::<_, Workspace>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("workspace {id}")))
}

pub async fn create(
    pool: &SqlitePool,
    paths: &AppPaths,
    input: CreateWorkspaceInput,
) -> AppResult<Workspace> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("workspace name is required".into()));
    }

    let root_path = canonicalize_workspace_path(&input.root_path, paths)?;
    let color = validate_color(input.color_hex.as_deref().unwrap_or("#3b82f6"))?;
    let policy = validate_policy(
        input
            .default_router_policy
            .as_deref()
            .unwrap_or("COST_BENEFIT"),
    )?;
    let timezone = input
        .timezone
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("UTC")
        .to_string();

    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO workspaces (
            id, name, root_path, description, color_hex, default_router_policy,
            default_email_recipient, google_drive_folder_id, timezone
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(&root_path)
    .bind(input.description.as_deref())
    .bind(&color)
    .bind(&policy)
    .bind(input.default_email_recipient.as_deref())
    .bind(input.google_drive_folder_id.as_deref())
    .bind(&timezone)
    .execute(pool)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            AppError::Conflict("a workspace with this root path already exists".to_string())
        }
        other => AppError::Db(other),
    })?;

    get(pool, &id).await
}

pub async fn update(
    pool: &SqlitePool,
    paths: &AppPaths,
    id: &str,
    input: UpdateWorkspaceInput,
) -> AppResult<Workspace> {
    let current = get(pool, id).await?;

    let name = match input.name {
        Some(name) => {
            let trimmed = name.trim().to_string();
            if trimmed.is_empty() {
                return Err(AppError::BadRequest(
                    "workspace name cannot be empty".into(),
                ));
            }
            trimmed
        }
        None => current.name,
    };

    let root_path = match input.root_path {
        Some(path) => canonicalize_workspace_path(&path, paths)?,
        None => current.root_path,
    };

    let color = match input.color_hex {
        Some(color) => validate_color(&color)?,
        None => current.color_hex,
    };

    let policy = match input.default_router_policy {
        Some(policy) => validate_policy(&policy)?,
        None => current.default_router_policy,
    };

    let timezone = match input.timezone {
        Some(timezone) if !timezone.trim().is_empty() => timezone.trim().to_string(),
        Some(_) => return Err(AppError::BadRequest("timezone cannot be empty".into())),
        None => current.timezone,
    };

    sqlx::query(
        "UPDATE workspaces SET
            name = ?, root_path = ?, description = ?, color_hex = ?,
            default_router_policy = ?, default_email_recipient = ?,
            google_drive_folder_id = ?, timezone = ?
         WHERE id = ?",
    )
    .bind(&name)
    .bind(&root_path)
    .bind(input.description.or(current.description))
    .bind(&color)
    .bind(&policy)
    .bind(
        input
            .default_email_recipient
            .or(current.default_email_recipient),
    )
    .bind(
        input
            .google_drive_folder_id
            .or(current.google_drive_folder_id),
    )
    .bind(&timezone)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            AppError::Conflict("another workspace already uses this root path".to_string())
        }
        other => AppError::Db(other),
    })?;

    get(pool, id).await
}

pub async fn touch(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query(
        "UPDATE workspaces SET last_opened_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("workspace {id}")));
    }
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM workspaces WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("workspace {id}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, db::settings, error::AppError, paths::AppPaths};
    use sqlx::SqlitePool;
    use tempfile::TempDir;

    fn test_paths(root: &std::path::Path) -> AppPaths {
        AppPaths {
            data_dir: root.join("data"),
            config_dir: root.join("config"),
            cache_dir: root.join("cache"),
            log_dir: root.join("logs"),
            db_path: root.join("data").join("opendesk.db"),
            backups_dir: root.join("backups"),
            artifacts_dir: root.join("artifacts"),
            run_logs_dir: root.join("runs"),
            daemon_lock_path: root.join("daemon.json"),
            api_lock_path: root.join("api.json"),
        }
    }

    async fn setup() -> (TempDir, SqlitePool, AppPaths) {
        let temp = TempDir::new().expect("temp dir");
        let paths = test_paths(temp.path());
        std::fs::create_dir_all(&paths.data_dir).expect("data dir");
        let pool = db::init(&paths.db_path).await.expect("database");
        (temp, pool, paths)
    }

    fn input(root: &std::path::Path, name: &str) -> CreateWorkspaceInput {
        CreateWorkspaceInput {
            name: name.to_string(),
            root_path: root.to_string_lossy().to_string(),
            description: Some("test workspace".into()),
            color_hex: Some("#ABCDEF".into()),
            default_router_policy: Some("COST_BENEFIT".into()),
            default_email_recipient: None,
            google_drive_folder_id: None,
            timezone: Some("America/Sao_Paulo".into()),
        }
    }

    #[tokio::test]
    async fn creates_native_workspace_layout() {
        let (temp, pool, paths) = setup().await;
        let root = temp.path().join("workspace");
        let workspace = create(&pool, &paths, input(&root, "Workspace"))
            .await
            .expect("create");

        let canonical = std::fs::canonicalize(&root).expect("canonical root");
        assert_eq!(workspace.root_path, canonical.to_string_lossy());
        assert_eq!(workspace.color_hex, "#abcdef");
        assert_eq!(workspace.timezone, "America/Sao_Paulo");
        assert!(root.join("input").is_dir());
        assert!(root.join("output").is_dir());
        assert!(root.join(".opencode").is_dir());

        let listed = list(&pool).await.expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, workspace.id);
    }

    #[tokio::test]
    async fn rejects_duplicate_root_and_invalid_fields() {
        let (temp, pool, paths) = setup().await;
        let root = temp.path().join("workspace");
        create(&pool, &paths, input(&root, "First"))
            .await
            .expect("first create");

        let duplicate = create(&pool, &paths, input(&root, "Second")).await;
        assert!(matches!(duplicate, Err(AppError::Conflict(_))));

        let mut invalid = input(&temp.path().join("other"), "Other");
        invalid.default_router_policy = Some("NOT_A_POLICY".into());
        let error = create(&pool, &paths, invalid).await;
        assert!(matches!(error, Err(AppError::BadRequest(_))));

        let mut invalid_color = input(&temp.path().join("other2"), "Other 2");
        invalid_color.color_hex = Some("blue".into());
        let error = create(&pool, &paths, invalid_color).await;
        assert!(matches!(error, Err(AppError::BadRequest(_))));
    }

    #[tokio::test]
    async fn updates_touches_and_deletes_workspace() {
        let (temp, pool, paths) = setup().await;
        let root = temp.path().join("workspace");
        let workspace = create(&pool, &paths, input(&root, "Original"))
            .await
            .expect("create");

        let updated = update(
            &pool,
            &paths,
            &workspace.id,
            UpdateWorkspaceInput {
                name: Some("Renamed".into()),
                root_path: None,
                description: None,
                color_hex: Some("#112233".into()),
                default_router_policy: Some("BEST_QUALITY".into()),
                default_email_recipient: None,
                google_drive_folder_id: None,
                timezone: Some("UTC".into()),
            },
        )
        .await
        .expect("update");

        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.color_hex, "#112233");
        assert_eq!(updated.default_router_policy, "BEST_QUALITY");

        touch(&pool, &workspace.id).await.expect("touch");
        delete(&pool, &workspace.id).await.expect("delete");
        assert!(matches!(
            get(&pool, &workspace.id).await,
            Err(AppError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn stores_active_workspace_setting() {
        let (temp, pool, paths) = setup().await;
        let workspace = create(&pool, &paths, input(&temp.path().join("ws"), "WS"))
            .await
            .expect("create");

        settings::set_active_workspace(&pool, &workspace.id)
            .await
            .expect("set active");
        assert_eq!(
            settings::active_workspace_id(&pool).await.expect("active"),
            Some(workspace.id)
        );
    }
}
