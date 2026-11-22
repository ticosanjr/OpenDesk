use crate::context::RecipeContext;
use crate::parser::Recipe;
use crate::store;

use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub(super) struct Ctx<'a> {
    pub(super) context: &'a RecipeContext,
    pub(super) recipe: &'a Recipe,
    pub(super) run: &'a store::RecipeRun,
    pub(super) task_id: &'a str,
    pub(super) trigger: &'a Value,
    pub(super) run_vars: &'a Value,
    pub(super) routine_id: Option<&'a str>,
    pub(super) dispatcher: &'a dyn opendesk_engine::recipe_dispatch::RecipeStepDispatcher,
}

#[derive(Default)]
pub(super) struct ScopeState {
    pub(super) outputs: Value,
    pub(super) session_id: Option<String>,
    pub(super) feedback: HashMap<String, String>,
    pub(super) review_rounds: usize,
    pub(super) recipe_stack: Vec<String>,
    pub(super) spent_usd: f64,
    pub(super) budget_usd: Option<f64>,
}

pub(super) enum ScopeOutcome {
    Continue,
    Cancelled,
}

pub(super) struct MapItemResult {
    pub(super) order: usize,
    pub(super) key: String,
    pub(super) item: Value,
    pub(super) outputs: Value,
    pub(super) spent_usd: f64,
}
