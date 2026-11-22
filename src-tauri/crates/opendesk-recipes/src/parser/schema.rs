use super::expressions::checkpoint_id;
use super::{FlowItem, Recipe};
use crate::error::{AppError, AppResult};
use opendesk_domain::HitlKind;

const MAX_DAG_DEPTH: usize = 8;
const MAX_DAG_NODES: usize = 500;
const MAX_PARALLEL_BRANCHES: usize = 16;
const MAX_LOOP_ITERATIONS: usize = 100;
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
                    if let Some(budget) = call.budget_usd
                        && (!budget.is_finite() || budget <= 0.0)
                    {
                        return Err(AppError::BadRequest(format!(
                            "budget_usd da sub-receita {} precisa ser positivo",
                            call.id
                        )));
                    }
                }
                FlowItem::Loop(loop_flow) => {
                    self.group_id(&loop_flow.id, "loop")?;
                    if !(1..=MAX_LOOP_ITERATIONS).contains(&loop_flow.max_iterations) {
                        return Err(AppError::BadRequest(format!(
                            "max_iterations do loop {} deve ficar entre 1 e {MAX_LOOP_ITERATIONS}",
                            loop_flow.id
                        )));
                    }
                    if loop_flow.flow.is_empty() {
                        return Err(AppError::BadRequest(format!(
                            "loop {} precisa de flow",
                            loop_flow.id
                        )));
                    }
                    self.collect(&loop_flow.flow, depth + 1, group)?;
                }
                FlowItem::Switch(switch) => {
                    self.group_id(&switch.id, "switch")?;
                    if switch.cases.is_empty() && switch.otherwise.is_empty() {
                        return Err(AppError::BadRequest(format!(
                            "switch {} precisa de cases ou default",
                            switch.id
                        )));
                    }
                    for case in &switch.cases {
                        if case.when.trim().is_empty() {
                            return Err(AppError::BadRequest(format!(
                                "case do switch {} precisa de when",
                                switch.id
                            )));
                        }
                        self.collect(&case.flow, depth + 1, group)?;
                    }
                    self.collect(&switch.otherwise, depth + 1, group)?;
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
                FlowItem::Loop(loop_flow) => self.check_references(&loop_flow.flow)?,
                FlowItem::Switch(switch) => {
                    for case in &switch.cases {
                        self.check_references(&case.flow)?;
                    }
                    self.check_references(&switch.otherwise)?;
                }
                FlowItem::Recipe(_) | FlowItem::Step(_) | FlowItem::Sink(_) => {}
            }
        }
        Ok(())
    }
}
