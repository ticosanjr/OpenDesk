use serde::Deserialize;

mod crud;
mod run;
mod triggers;
mod validate;

pub use crud::*;
pub use run::*;
pub(crate) use triggers::sync_recipe_trigger_sources;
pub use triggers::{sync, sync_all_recipe_trigger_sources};
pub use validate::*;

#[derive(Debug, Deserialize, Default)]
pub struct RecipeQuery {
    pub workspace_id: Option<String>,
}
