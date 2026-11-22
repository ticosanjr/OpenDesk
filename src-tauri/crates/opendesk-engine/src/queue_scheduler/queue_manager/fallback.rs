use super::QueueManager;
use opendesk_storage::repos::{model_health, router as router_db};

use std::path::Path;

impl QueueManager {
    pub(super) async fn pick_fallback_model(
        &self,
        workspace_id: &str,
        workspace_root: &Path,
        current: &str,
    ) -> Option<String> {
        let models = self
            .daemon
            .models(Some(&workspace_root.to_string_lossy()))
            .await
            .ok()?;
        let cooling = model_health::cooling(&self.pool, 24)
            .await
            .unwrap_or_default();
        let current_base = current.split('#').next().unwrap_or(current).to_string();
        let key = |model: &crate::opencode_client::rest_client::ModelDto| {
            let provider = model.provider_id.clone().unwrap_or_default();
            let id = model.model_id.clone().unwrap_or_else(|| model.id.clone());
            format!("{provider}/{id}")
        };
        let router_models = router_db::models(&self.pool, workspace_id)
            .await
            .unwrap_or_default();
        let mut preferred = Vec::new();
        if let Some(model) = router_models.default_model.as_deref() {
            preferred.push(model);
        }
        if let Some(model) = router_models.manual_model.as_deref() {
            preferred.push(model);
        }
        preferred.extend(["opencode-go/deepseek-v4.1-flash", "opencode-go/glm-5.3"]);
        let mut candidates = models
            .iter()
            .filter(|model| model.enabled.unwrap_or(true))
            .filter(|model| {
                let name = key(model);
                name != current_base && !cooling.contains(&name)
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|model| {
            let name = key(model);
            preferred
                .iter()
                .position(|candidate| *candidate == name)
                .unwrap_or(usize::MAX)
        });
        let model = candidates.first()?;
        let base = key(model);
        if model.variants.iter().any(|variant| variant.id == "low") {
            Some(format!("{base}#low"))
        } else {
            Some(base)
        }
    }
}
