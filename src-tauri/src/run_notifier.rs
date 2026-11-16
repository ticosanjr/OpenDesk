use crate::{
    error::AppResult,
    integrations,
    notifier::{PermissionNotice, RunNotifier},
};
use async_trait::async_trait;

/// Shell-side delivery for permission requests. Network sends are spawned so a
/// slow channel never blocks the run's permission loop.
pub struct ShellRunNotifier;

#[async_trait]
impl RunNotifier for ShellRunNotifier {
    async fn permission_requested(&self, notice: PermissionNotice<'_>) -> AppResult<()> {
        let PermissionNotice {
            pool,
            secrets,
            workspace_id,
            run_id,
            action,
            resources,
            request_id,
        } = notice;
        let _ = (workspace_id, run_id);
        if let Ok(config) = integrations::signal::load(pool).await
            && config.enabled
            && !config.account.trim().is_empty()
        {
            let text = format!(
                "OpenDesk aguardando decisão\nAção: {action}\nRecursos: {}\nResponda: APROVAR {} ou NEGAR {}",
                resources.join(", "),
                &request_id[..request_id.len().min(12)],
                &request_id[..request_id.len().min(12)],
            );
            for recipient in config.allowlist.clone() {
                let cli = config.cli_path.clone();
                let account = config.account.clone();
                let text = text.clone();
                tokio::spawn(async move {
                    let _ =
                        integrations::signal::send_text(&cli, &account, &recipient, &text).await;
                });
            }
        }

        if let Ok(config) = integrations::telegram::load(pool).await
            && config.enabled
            && integrations::telegram::token(secrets).is_some()
        {
            let pool = pool.clone();
            let secrets = secrets.clone();
            let action = action.to_string();
            let resources = resources.to_vec();
            let request_id = request_id.to_string();
            tokio::spawn(async move {
                let _ = integrations::telegram::notify_permission(
                    &pool,
                    &secrets,
                    &action,
                    &resources,
                    &request_id,
                )
                .await;
            });
        }
        Ok(())
    }
}
