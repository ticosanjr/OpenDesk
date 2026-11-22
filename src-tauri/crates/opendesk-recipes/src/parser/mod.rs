//! Recipe YAML: types, validation, template rendering and output schemas.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

mod expressions;
mod schema;
#[cfg(test)]
include!("tests.rs");

pub use expressions::{
    checkpoint_id, evaluate_when, extract_json, node_descriptor, render, validate_output,
};
pub use schema::validate;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Legacy single trigger. Kept for old YAML files and mirrored into
    /// `triggers` during normalization.
    #[serde(default)]
    pub trigger: Trigger,
    /// Zero or more triggers. An empty list means the recipe is only callable
    /// from routines or manually.
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    /// Legacy step list, kept for compatibility. Normalized into `flow`.
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub hitl_checkpoints: Vec<HitlCheckpoint>,
    #[serde(default)]
    pub sinks: Vec<RecipeSink>,
    /// Canonical ordered execution flow: steps, checkpoints and sinks can be
    /// interleaved in any order.
    #[serde(default)]
    pub flow: Vec<FlowItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Trigger {
    #[serde(rename = "type", default)]
    pub trigger_type: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub name: String,
    #[serde(default = "default_build")]
    pub mode: String,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub prompt: String,
    /// Optional condition. Empty/true always runs; false skips the step.
    #[serde(default)]
    pub when: Option<String>,
    /// Optional JSON Schema for the step input context.
    #[serde(default)]
    pub input_schema: Option<serde_json::Value>,
    /// Optional JSON Schema the agent output must satisfy.
    #[serde(default)]
    pub output_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitlCheckpoint {
    #[serde(default)]
    pub id: String,
    /// Legacy positional hint. In the unified flow the checkpoint position is
    /// defined by the list order, so this is optional.
    #[serde(default)]
    pub after_step: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub interaction_type: String,
    #[serde(default)]
    pub channels: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_minutes: i64,
    #[serde(default = "default_on_timeout")]
    pub on_timeout: String,
    pub prompt_template: String,
    #[serde(default)]
    pub attach_files: Vec<String>,
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeSink {
    #[serde(rename = "type")]
    pub sink_type: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowSink {
    #[serde(rename = "sink_type", alias = "sinkType")]
    pub sink_type: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub config: serde_json::Value,
}

/// Conditional group. `when` is evaluated against `trigger`, `run` and the
/// outputs visible at this point; only the selected list runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchFlow {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub when: String,
    #[serde(default)]
    pub then: Vec<FlowItem>,
    #[serde(default, rename = "else")]
    pub otherwise: Vec<FlowItem>,
}

/// Fixed fan-out. Branches run concurrently and the group finishes according
/// to `join` (`all` or `any`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelFlow {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default = "default_join")]
    pub join: String,
    #[serde(default)]
    pub branches: Vec<Vec<FlowItem>>,
}

/// Dynamic fan-out over a JSON array. Each item gets its own isolated outputs
/// plus `items[item_var]` (and `steps[item_var]`) for templates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapFlow {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    /// Expression rendered to a JSON array, e.g. `{{steps.list.structured.files}}`.
    pub items: String,
    #[serde(default = "default_item_var")]
    pub item_var: String,
    #[serde(default = "default_map_concurrency")]
    pub max_concurrency: usize,
    #[serde(default)]
    pub flow: Vec<FlowItem>,
}

/// Calls another recipe in the same workspace with isolated outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeCall {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    /// Recipe id (slug) inside the same workspace.
    pub recipe: String,
    #[serde(default, rename = "with")]
    pub inputs: serde_json::Value,
    #[serde(default)]
    pub when: Option<String>,
    /// Optional USD cap for the sub-recipe's own steps.
    #[serde(default)]
    pub budget_usd: Option<f64>,
}

/// Bounded loop. Each iteration gets its own durable node path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopFlow {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default = "default_loop_iterations")]
    pub max_iterations: usize,
    #[serde(default)]
    pub until: Option<String>,
    pub flow: Vec<FlowItem>,
}

/// Multi-way branch. The first `when` that evaluates true wins; `default` is
/// used when none matches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchCase {
    pub when: String,
    #[serde(default)]
    pub flow: Vec<FlowItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchFlow {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub cases: Vec<SwitchCase>,
    #[serde(default, rename = "default")]
    pub otherwise: Vec<FlowItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FlowItem {
    Step(Step),
    Hitl(HitlCheckpoint),
    Sink(FlowSink),
    Branch(BranchFlow),
    Parallel(ParallelFlow),
    Map(MapFlow),
    Recipe(RecipeCall),
    Loop(LoopFlow),
    Switch(SwitchFlow),
}

impl From<RecipeSink> for FlowSink {
    fn from(sink: RecipeSink) -> Self {
        Self {
            sink_type: sink.sink_type,
            name: sink.name,
            description: sink.description,
            config: sink.config,
        }
    }
}

impl FlowItem {
    pub fn is_dag(&self) -> bool {
        matches!(
            self,
            Self::Branch(_)
                | Self::Parallel(_)
                | Self::Map(_)
                | Self::Recipe(_)
                | Self::Loop(_)
                | Self::Switch(_)
        )
    }

    pub fn collect_steps<'a>(&'a self, steps: &mut Vec<&'a Step>) {
        match self {
            Self::Step(step) => steps.push(step),
            Self::Hitl(_) | Self::Sink(_) => {}
            Self::Branch(branch) => {
                for item in branch.then.iter().chain(branch.otherwise.iter()) {
                    item.collect_steps(steps);
                }
            }
            Self::Parallel(parallel) => {
                for branch in &parallel.branches {
                    for item in branch {
                        item.collect_steps(steps);
                    }
                }
            }
            Self::Map(map) => {
                for item in &map.flow {
                    item.collect_steps(steps);
                }
            }
            Self::Recipe(_) => {}
            Self::Loop(loop_flow) => {
                for item in &loop_flow.flow {
                    item.collect_steps(steps);
                }
            }
            Self::Switch(switch) => {
                for case in &switch.cases {
                    for item in &case.flow {
                        item.collect_steps(steps);
                    }
                }
                for item in &switch.otherwise {
                    item.collect_steps(steps);
                }
            }
        }
    }
}

impl From<FlowSink> for RecipeSink {
    fn from(sink: FlowSink) -> Self {
        Self {
            sink_type: sink.sink_type,
            name: sink.name,
            description: sink.description,
            config: sink.config,
        }
    }
}

impl Recipe {
    /// All active triggers. Legacy `trigger` is promoted when `triggers` is
    /// empty, and an entirely empty trigger set is valid.
    pub fn triggers(&self) -> Vec<&Trigger> {
        if self.triggers.is_empty() {
            if self.trigger.trigger_type.trim().is_empty() {
                Vec::new()
            } else {
                vec![&self.trigger]
            }
        } else {
            self.triggers.iter().collect()
        }
    }

    pub fn primary_trigger_type(&self) -> String {
        self.triggers()
            .first()
            .map(|trigger| trigger.trigger_type.clone())
            .unwrap_or_else(|| "MANUAL".into())
    }

    pub fn steps(&self) -> impl Iterator<Item = &Step> {
        self.flow.iter().filter_map(|item| match item {
            FlowItem::Step(step) => Some(step),
            _ => None,
        })
    }

    #[allow(dead_code)]
    pub fn step_count(&self) -> usize {
        self.steps().count()
    }

    /// True when the flow needs the path-based DAG executor.
    pub fn has_dag(&self) -> bool {
        self.flow.iter().any(FlowItem::is_dag)
    }

    /// All steps, including ones nested in branch/parallel/map groups.
    pub fn all_steps(&self) -> Vec<&Step> {
        let mut steps = Vec::new();
        for item in &self.flow {
            item.collect_steps(&mut steps);
        }
        steps
    }

    fn normalize(&mut self) {
        if self.triggers.is_empty() && !self.trigger.trigger_type.trim().is_empty() {
            self.triggers.push(self.trigger.clone());
        }
        if self.flow.is_empty() {
            for step in &self.steps {
                self.flow.push(FlowItem::Step(step.clone()));
                for checkpoint in self
                    .hitl_checkpoints
                    .iter()
                    .filter(|checkpoint| checkpoint.after_step == step.id)
                {
                    self.flow.push(FlowItem::Hitl(checkpoint.clone()));
                }
            }
            for checkpoint in self.hitl_checkpoints.iter().filter(|checkpoint| {
                checkpoint.after_step.trim().is_empty()
                    || !self
                        .steps
                        .iter()
                        .any(|step| step.id == checkpoint.after_step)
            }) {
                self.flow.push(FlowItem::Hitl(checkpoint.clone()));
            }
            for sink in &self.sinks {
                self.flow.push(FlowItem::Sink(sink.clone().into()));
            }
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_build() -> String {
    "build".into()
}
fn default_timeout() -> i64 {
    240
}
fn default_on_timeout() -> String {
    "PAUSE_AND_ALERT".into()
}
fn default_join() -> String {
    "all".into()
}
fn default_item_var() -> String {
    "item".into()
}
fn default_map_concurrency() -> usize {
    3
}
fn default_loop_iterations() -> usize {
    3
}

pub fn parse(text: &str) -> AppResult<Recipe> {
    let mut recipe: Recipe = serde_yaml::from_str(text)
        .map_err(|error| AppError::BadRequest(format!("YAML inválido: {error}")))?;
    recipe.normalize();
    validate(&recipe)?;
    Ok(recipe)
}
