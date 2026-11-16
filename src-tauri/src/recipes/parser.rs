use crate::{
    error::{AppError, AppResult},
    hitl::HitlKind,
};
use serde::{Deserialize, Serialize};

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
            Self::Branch(_) | Self::Parallel(_) | Self::Map(_) | Self::Recipe(_)
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

pub fn parse(text: &str) -> AppResult<Recipe> {
    let mut recipe: Recipe = serde_yaml::from_str(text)
        .map_err(|error| AppError::BadRequest(format!("YAML inválido: {error}")))?;
    recipe.normalize();
    validate(&recipe)?;
    Ok(recipe)
}

const MAX_DAG_DEPTH: usize = 8;
const MAX_DAG_NODES: usize = 500;
const MAX_PARALLEL_BRANCHES: usize = 16;
const MAX_MAP_CONCURRENCY: usize = 8;

struct Validator {
    step_ids: std::collections::HashSet<String>,
    checkpoint_ids: std::collections::HashSet<String>,
    group_ids: std::collections::HashSet<String>,
    step_count: usize,
    node_count: usize,
}

pub fn validate(recipe: &Recipe) -> AppResult<()> {
    if recipe.id.trim().is_empty() {
        return Err(AppError::BadRequest("recipe.id é obrigatório".into()));
    }
    if recipe.name.trim().is_empty() {
        return Err(AppError::BadRequest("recipe.name é obrigatório".into()));
    }
    let mut validator = Validator {
        step_ids: Default::default(),
        checkpoint_ids: Default::default(),
        group_ids: Default::default(),
        step_count: 0,
        node_count: 0,
    };
    validator.collect(&recipe.flow, 0, None)?;
    if validator.step_count == 0 {
        return Err(AppError::BadRequest(
            "a receita precisa de ao menos um step".into(),
        ));
    }
    validator.check_references(&recipe.flow)?;
    Ok(())
}

impl Validator {
    fn collect(&mut self, items: &[FlowItem], depth: usize, group: Option<&str>) -> AppResult<()> {
        if depth > MAX_DAG_DEPTH {
            return Err(AppError::BadRequest(format!(
                "fluxo aninhado demais (máximo {MAX_DAG_DEPTH} níveis)"
            )));
        }
        for item in items {
            self.node_count += 1;
            if self.node_count > MAX_DAG_NODES {
                return Err(AppError::BadRequest(format!(
                    "receita grande demais (máximo {MAX_DAG_NODES} nós)"
                )));
            }
            match item {
                FlowItem::Step(step) => {
                    self.step_count += 1;
                    if step.id.trim().is_empty() || !self.step_ids.insert(step.id.clone()) {
                        return Err(AppError::BadRequest(format!(
                            "step id duplicado ou vazio: {}",
                            step.id
                        )));
                    }
                    if !matches!(step.mode.as_str(), "build" | "plan" | "audit") {
                        return Err(AppError::BadRequest(format!(
                            "modo de step inválido '{}'",
                            step.mode
                        )));
                    }
                    if step.prompt.trim().is_empty() {
                        return Err(AppError::BadRequest(format!(
                            "prompt vazio no step {}",
                            step.id
                        )));
                    }
                    for (field, schema) in [
                        ("input_schema", step.input_schema.as_ref()),
                        ("output_schema", step.output_schema.as_ref()),
                    ] {
                        if let Some(schema) = schema
                            && jsonschema::validator_for(schema).is_err()
                        {
                            return Err(AppError::BadRequest(format!(
                                "{field} inválido no step {}",
                                step.id
                            )));
                        }
                    }
                }
                FlowItem::Hitl(checkpoint) => {
                    if group.is_some() {
                        return Err(AppError::BadRequest(
                            "HITL não é permitido dentro de parallel/map; mova o checkpoint para o nível principal"
                                .into(),
                        ));
                    }
                    let id = checkpoint_id(checkpoint);
                    if !self.checkpoint_ids.insert(id.clone()) {
                        return Err(AppError::BadRequest(format!(
                            "checkpoint id duplicado: {id}"
                        )));
                    }
                    let kind = HitlKind::parse(&checkpoint.interaction_type).ok_or_else(|| {
                        AppError::BadRequest(format!(
                            "interaction_type inválido '{}'",
                            checkpoint.interaction_type
                        ))
                    })?;
                    if !matches!(
                        checkpoint.on_timeout.as_str(),
                        "PAUSE_AND_ALERT" | "AUTO_APPROVE" | "ABORT"
                    ) {
                        return Err(AppError::BadRequest(format!(
                            "on_timeout inválido '{}'",
                            checkpoint.on_timeout
                        )));
                    }
                    if kind.requires_options() && checkpoint.options.is_empty() {
                        return Err(AppError::BadRequest(format!(
                            "checkpoint {} {} precisa de options",
                            id,
                            kind.as_str()
                        )));
                    }
                    if checkpoint.prompt_template.trim().is_empty() {
                        return Err(AppError::BadRequest(format!(
                            "checkpoint {id} precisa de prompt_template"
                        )));
                    }
                }
                FlowItem::Sink(sink) => {
                    if group.is_some() {
                        return Err(AppError::BadRequest(
                            "sink não é permitido dentro de parallel/map; execute-o após o join"
                                .into(),
                        ));
                    }
                    if sink.sink_type.trim().is_empty() {
                        return Err(AppError::BadRequest(
                            "sink sem tipo (sink_type) definido".into(),
                        ));
                    }
                }
                FlowItem::Branch(branch) => {
                    self.group_id(&branch.id, "branch")?;
                    if branch.when.trim().is_empty() {
                        return Err(AppError::BadRequest(format!(
                            "branch {} precisa de when",
                            branch.id
                        )));
                    }
                    if branch.then.is_empty() && branch.otherwise.is_empty() {
                        return Err(AppError::BadRequest(format!(
                            "branch {} precisa de then ou else",
                            branch.id
                        )));
                    }
                    self.collect(&branch.then, depth + 1, group)?;
                    self.collect(&branch.otherwise, depth + 1, group)?;
                }
                FlowItem::Parallel(parallel) => {
                    self.group_id(&parallel.id, "parallel")?;
                    if !matches!(parallel.join.as_str(), "all" | "any") {
                        return Err(AppError::BadRequest(format!(
                            "join inválido no parallel {}: use all ou any",
                            parallel.id
                        )));
                    }
                    if parallel.branches.is_empty() {
                        return Err(AppError::BadRequest(format!(
                            "parallel {} precisa de ao menos um branch",
                            parallel.id
                        )));
                    }
                    if parallel.branches.len() > MAX_PARALLEL_BRANCHES {
                        return Err(AppError::BadRequest(format!(
                            "parallel {} excede {MAX_PARALLEL_BRANCHES} branches",
                            parallel.id
                        )));
                    }
                    for branch in &parallel.branches {
                        if branch.is_empty() {
                            return Err(AppError::BadRequest(format!(
                                "branch vazio no parallel {}",
                                parallel.id
                            )));
                        }
                        self.collect(branch, depth + 1, Some(&parallel.id))?;
                    }
                }
                FlowItem::Recipe(call) => {
                    if group.is_some() {
                        return Err(AppError::BadRequest(
                            "sub-receita não é permitida dentro de parallel/map; chame-a no nível principal"
                                .into(),
                        ));
                    }
                    self.group_id(&call.id, "recipe")?;
                    if call.recipe.trim().is_empty() {
                        return Err(AppError::BadRequest(format!(
                            "recipe call {} precisa de recipe",
                            call.id
                        )));
                    }
                    if !call.inputs.is_null() && !call.inputs.is_object() {
                        return Err(AppError::BadRequest(format!(
                            "with da sub-receita {} precisa ser um objeto",
                            call.id
                        )));
                    }
                }
                FlowItem::Map(map) => {
                    self.group_id(&map.id, "map")?;
                    if map.items.trim().is_empty() {
                        return Err(AppError::BadRequest(format!(
                            "map {} precisa de items",
                            map.id
                        )));
                    }
                    if map.item_var.trim().is_empty()
                        || !map
                            .item_var
                            .chars()
                            .all(|character| character.is_ascii_alphanumeric() || character == '_')
                    {
                        return Err(AppError::BadRequest(format!(
                            "item_var inválido no map {}",
                            map.id
                        )));
                    }
                    if !(1..=MAX_MAP_CONCURRENCY).contains(&map.max_concurrency) {
                        return Err(AppError::BadRequest(format!(
                            "max_concurrency do map {} deve ficar entre 1 e {MAX_MAP_CONCURRENCY}",
                            map.id
                        )));
                    }
                    if map.flow.is_empty() {
                        return Err(AppError::BadRequest(format!(
                            "map {} precisa de flow",
                            map.id
                        )));
                    }
                    self.collect(&map.flow, depth + 1, Some(&map.id))?;
                }
            }
        }
        Ok(())
    }

    fn group_id(&mut self, id: &str, kind: &str) -> AppResult<()> {
        if id.trim().is_empty() || !self.group_ids.insert(id.to_string()) {
            return Err(AppError::BadRequest(format!(
                "{kind} id duplicado ou vazio: {id}"
            )));
        }
        Ok(())
    }

    fn check_references(&self, items: &[FlowItem]) -> AppResult<()> {
        for item in items {
            match item {
                FlowItem::Hitl(checkpoint) => {
                    if !checkpoint.after_step.trim().is_empty()
                        && !self.step_ids.contains(&checkpoint.after_step)
                    {
                        return Err(AppError::BadRequest(format!(
                            "checkpoint {} aponta para step inexistente {}",
                            checkpoint.id, checkpoint.after_step
                        )));
                    }
                }
                FlowItem::Branch(branch) => {
                    self.check_references(&branch.then)?;
                    self.check_references(&branch.otherwise)?;
                }
                FlowItem::Parallel(parallel) => {
                    for branch in &parallel.branches {
                        self.check_references(branch)?;
                    }
                }
                FlowItem::Map(map) => self.check_references(&map.flow)?,
                FlowItem::Recipe(_) | FlowItem::Step(_) | FlowItem::Sink(_) => {}
            }
        }
        Ok(())
    }
}

/// Checkpoints use a generated stable id when the author leaves it empty.
/// Stable id/kind for durable recipe nodes/timeline.
pub fn node_descriptor(item: &FlowItem, index: usize) -> (String, &'static str) {
    match item {
        FlowItem::Step(step) => (step.id.clone(), "step"),
        FlowItem::Hitl(checkpoint) => (checkpoint_id(checkpoint), "hitl"),
        FlowItem::Sink(sink) => (
            sink.name
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("sink-{index}")),
            "sink",
        ),
        FlowItem::Branch(branch) => (branch.id.clone(), "branch"),
        FlowItem::Parallel(parallel) => (parallel.id.clone(), "parallel"),
        FlowItem::Map(map) => (map.id.clone(), "map"),
        FlowItem::Recipe(call) => (call.id.clone(), "recipe"),
    }
}

pub fn checkpoint_id(checkpoint: &HitlCheckpoint) -> String {
    if !checkpoint.id.trim().is_empty() {
        checkpoint.id.clone()
    } else {
        format!("hitl-{}", checkpoint.interaction_type.to_lowercase())
    }
}

/// Extracts the last fenced JSON block (or the first object) from an agent
/// response and parses it.
pub fn extract_json(summary: &str) -> Option<serde_json::Value> {
    let mut fenced = None;
    let mut rest = summary;
    while let Some(start) = rest.find("```") {
        let after = &rest[start + 3..];
        let after = after.strip_prefix("json").unwrap_or(after);
        let Some(end) = after.find("```") else {
            break;
        };
        let candidate = after[..end].trim();
        if let Ok(value) = serde_json::from_str(candidate) {
            fenced = Some(value);
        }
        rest = &after[end + 3..];
    }
    if fenced.is_some() {
        return fenced;
    }
    let start = summary.find('{')?;
    let end = summary.rfind('}')?;
    serde_json::from_str(summary[start..=end].trim()).ok()
}

pub fn validate_output(schema: &serde_json::Value, summary: &str) -> AppResult<serde_json::Value> {
    let value = extract_json(summary)
        .ok_or_else(|| AppError::BadRequest("a saída do step não contém um JSON válido".into()))?;
    let validator = jsonschema::validator_for(schema)
        .map_err(|error| AppError::BadRequest(format!("schema inválido: {error}")))?;
    validator
        .validate(&value)
        .map_err(|error| AppError::BadRequest(format!("saída fora do schema: {error}")))?;
    Ok(value)
}

/// Resolve {{...}} variables and fail closed on unknown names.
pub fn render(
    template: &str,
    trigger: &serde_json::Value,
    run: &serde_json::Value,
    steps: &serde_json::Value,
) -> AppResult<String> {
    let mut output = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        output.push_str(&rest[..start]);
        let Some(end) = rest[start + 2..].find("}}") else {
            output.push_str(&rest[start..]);
            return Ok(output);
        };
        let expression = rest[start + 2..start + 2 + end].trim();
        let value = resolve(expression, trigger, run, steps)
            .ok_or_else(|| AppError::BadRequest(format!("variável desconhecida: {expression}")))?;
        output.push_str(&value);
        rest = &rest[start + 2 + end + 2..];
    }
    output.push_str(rest);
    Ok(output)
}

pub fn evaluate_when(
    expression: &str,
    trigger: &serde_json::Value,
    run: &serde_json::Value,
    steps: &serde_json::Value,
) -> AppResult<bool> {
    let expression = expression.trim();
    if expression.is_empty() || expression.eq_ignore_ascii_case("true") {
        return Ok(true);
    }
    if expression.eq_ignore_ascii_case("false") {
        return Ok(false);
    }
    for operator in ["!=", "=="] {
        if let Some((left, right)) = expression.split_once(operator) {
            let left = resolve(left.trim(), trigger, run, steps).unwrap_or_default();
            let right = right
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            let equal = left.eq_ignore_ascii_case(&right);
            return Ok(if operator == "==" { equal } else { !equal });
        }
    }
    if let Some(path) = expression.strip_prefix("not ") {
        let value = resolve(path.trim(), trigger, run, steps).unwrap_or_default();
        return Ok(!truthy(&value));
    }
    let value = resolve(expression, trigger, run, steps).unwrap_or_default();
    Ok(truthy(&value))
}

fn truthy(value: &str) -> bool {
    !matches!(
        value.trim().to_lowercase().as_str(),
        "" | "false" | "0" | "null" | "none"
    )
}

fn resolve(
    expression: &str,
    trigger: &serde_json::Value,
    run: &serde_json::Value,
    steps: &serde_json::Value,
) -> Option<String> {
    let (root, path) = if let Some(path) = expression.strip_prefix("trigger.") {
        (trigger, path)
    } else if let Some(path) = expression.strip_prefix("run.") {
        (run, path)
    } else if let Some(path) = expression.strip_prefix("steps.") {
        (steps, path)
    } else {
        // Inside map groups the item variable is also exposed at the top level
        // (`{{arquivo.path}}`) in addition to `{{steps.arquivo.path}}`.
        (steps, expression)
    };
    let pointer = format!("/{}", path.replace('.', "/"));
    let value = root.pointer(&pointer)?;
    Some(match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
id: teste
name: Teste
trigger:
  type: FOLDER_WATCH
  config: { path: input/dropzone }
steps:
  - id: s1
    name: Step 1
    prompt: "Analise {{trigger.file_path}}"
hitl_checkpoints:
  - id: h1
    after_step: s1
    interaction_type: BINARY_GATE
    prompt_template: "Aprovar?"
sinks:
  - type: DOC_COMPILER
    config: { format: pdf }
"#;

    #[test]
    fn resolves_top_level_map_variables() {
        let steps = serde_json::json!({
            "arquivo": { "path": "input/a.txt" },
            "list": { "structured": { "files": ["a", "b"] } }
        });
        assert_eq!(
            render(
                "Analise {{arquivo.path}}",
                &serde_json::Value::Null,
                &serde_json::Value::Null,
                &steps
            )
            .expect("render"),
            "Analise input/a.txt"
        );
    }

    #[test]
    fn parses_and_validates_dag_groups() {
        let yaml = r#"
id: dag
name: DAG
flow:
  - type: branch
    id: decidir
    when: steps.plan.structured.status == "ok"
    then:
      - type: step
        id: t1
        name: Then
        prompt: ok
    else:
      - type: step
        id: e1
        name: Else
        prompt: fallback
  - type: parallel
    id: lote
    join: all
    branches:
      - - type: step
          id: p1
          name: P1
          prompt: a
      - - type: step
          id: p2
          name: P2
          prompt: b
  - type: map
    id: arquivos
    items: "{{steps.list.structured.files}}"
    item_var: arquivo
    max_concurrency: 2
    flow:
      - type: step
        id: analisar
        name: Analisar
        prompt: "Analise {{arquivo.path}}"
"#;
        let recipe = parse(yaml).expect("parse");
        assert!(recipe.has_dag());
        assert_eq!(recipe.all_steps().len(), 5);
        let yaml = serde_yaml::to_string(&recipe).expect("serialize");
        parse(&yaml).expect("round trip");

        let invalid = r#"
id: bad
name: Bad
flow:
  - type: parallel
    id: p
    branches:
      - - type: hitl
          interaction_type: BINARY_GATE
          prompt_template: ok
  - type: step
    id: s
    name: S
    prompt: x
"#;
        assert!(parse(invalid).is_err(), "HITL inside parallel must fail");

        let invalid = r#"
id: bad2
name: Bad2
flow:
  - type: map
    id: m
    items: "[]"
    max_concurrency: 99
    flow:
      - type: step
        id: s
        name: S
        prompt: x
"#;
        assert!(parse(invalid).is_err(), "concurrency must be bounded");
    }

    #[test]
    fn parses_recipe_calls() {
        let yaml = r#"
id: caller
name: Caller
flow:
  - type: recipe
    id: chamar
    recipe: child
    with:
      topic: "{{trigger.topic}}"
  - type: step
    id: s
    name: S
    prompt: x
"#;
        let recipe = parse(yaml).expect("parse");
        assert!(recipe.has_dag());
        let yaml = serde_yaml::to_string(&recipe).expect("serialize");
        let parsed = parse(&yaml).expect("round trip");
        assert!(matches!(parsed.flow[0], FlowItem::Recipe(_)));
    }

    #[test]
    fn evaluates_when_conditions() {
        let trigger = serde_json::json!({ "approved": true, "name": "Ana" });
        let steps = serde_json::json!({
            "plan": { "structured": { "status": "ok", "count": 2 } }
        });
        assert!(evaluate_when("", &trigger, &serde_json::Value::Null, &steps).expect("empty"));
        assert!(
            evaluate_when(
                "trigger.approved",
                &trigger,
                &serde_json::Value::Null,
                &steps
            )
            .expect("path")
        );
        assert!(
            !evaluate_when(
                "trigger.missing",
                &trigger,
                &serde_json::Value::Null,
                &steps
            )
            .expect("missing")
        );
        assert!(
            evaluate_when(
                "steps.plan.structured.status == \"ok\"",
                &trigger,
                &serde_json::Value::Null,
                &steps
            )
            .expect("eq")
        );
        assert!(
            !evaluate_when(
                "steps.plan.structured.status != \"ok\"",
                &trigger,
                &serde_json::Value::Null,
                &steps
            )
            .expect("neq")
        );
        assert!(
            evaluate_when(
                "not trigger.missing",
                &trigger,
                &serde_json::Value::Null,
                &steps
            )
            .expect("not")
        );
    }

    #[test]
    fn extracts_and_validates_json_contracts() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["title", "count"],
            "properties": {
                "title": { "type": "string" },
                "count": { "type": "integer", "minimum": 1 }
            }
        });
        let summary = "Segue o resultado:\n```json\n{\"title\":\"x\",\"count\":2}\n```";
        let value = validate_output(&schema, summary).expect("valid");
        assert_eq!(value["count"], 2);
        assert!(validate_output(&schema, "sem json").is_err());
        assert!(validate_output(&schema, "{\"title\":\"x\",\"count\":0}").is_err());
    }

    #[test]
    fn rejects_invalid_step_schemas() {
        let yaml = "id: schema\nname: Schema\ntrigger:\n  type: MANUAL\n  config: {}\nflow:\n  - type: step\n    id: s1\n    name: Um\n    prompt: oi\n    output_schema: { type: nope }\n";
        assert!(parse(yaml).is_err());
    }

    #[test]
    fn parses_and_validates_sample() {
        let recipe = parse(SAMPLE).expect("recipe");
        assert_eq!(recipe.step_count(), 1);
        assert_eq!(recipe.primary_trigger_type(), "FOLDER_WATCH");
        assert_eq!(recipe.flow.len(), 3);
        assert!(matches!(recipe.flow[0], FlowItem::Step(_)));
        assert!(matches!(recipe.flow[1], FlowItem::Hitl(_)));
        assert!(matches!(recipe.flow[2], FlowItem::Sink(_)));
    }

    #[test]
    fn parses_unified_flow_and_multiple_triggers() {
        let yaml = r#"
id: flow
name: Flow
triggers:
  - type: FOLDER_WATCH
    config: { path: input }
  - type: WEBHOOK
    config: { secret: x }
flow:
  - type: step
    id: s1
    name: One
    prompt: primeiro
  - type: hitl
    id: h1
    interaction_type: BINARY_GATE
    prompt_template: ok?
  - type: step
    id: s2
    name: Two
    prompt: segundo
  - type: sink
    sink_type: DOC_COMPILER
    config: { format: pdf }
"#;
        let recipe = parse(yaml).expect("recipe");
        assert_eq!(recipe.triggers.len(), 2);
        assert_eq!(recipe.flow.len(), 4);
        assert!(matches!(recipe.flow[1], FlowItem::Hitl(_)));
        assert!(matches!(recipe.flow[3], FlowItem::Sink(_)));
    }

    #[test]
    fn accepts_no_triggers() {
        let yaml = r#"
id: routine-only
name: Routine only
flow:
  - type: step
    id: s1
    name: One
    prompt: execute
"#;
        let recipe = parse(yaml).expect("recipe");
        assert!(recipe.triggers().is_empty());
        assert_eq!(recipe.primary_trigger_type(), "MANUAL");
    }

    #[test]
    fn resolves_variables_and_rejects_unknown() {
        let rendered = render(
            "x {{trigger.file_path}} {{run.output_dir}} {{steps.s1.summary}}",
            &serde_json::json!({"file_path": "input/a.pdf"}),
            &serde_json::json!({"output_dir": "/tmp/out"}),
            &serde_json::json!({"s1": {"summary": "ok"}}),
        )
        .expect("render");
        assert_eq!(rendered, "x input/a.pdf /tmp/out ok");
        assert!(
            render(
                "{{trigger.missing}}",
                &serde_json::json!({}),
                &serde_json::json!({}),
                &serde_json::json!({})
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_bad_checkpoint() {
        let text = SAMPLE.replace("after_step: s1", "after_step: nope");
        assert!(parse(&text).is_err());
    }
}
