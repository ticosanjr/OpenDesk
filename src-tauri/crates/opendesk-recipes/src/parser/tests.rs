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
    fn rejects_invalid_sub_recipe_budget() {
        let yaml = r#"
id: budget
name: Budget
flow:
  - type: recipe
    id: chamar
    recipe: child
    budget_usd: 0
  - type: step
    id: s
    name: S
    prompt: x
"#;
        assert!(parse(yaml).is_err());
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
    #[test]
    fn parses_loop_and_switch_groups() {
        let yaml = r#"
id: loop-switch
name: Loop
flow:
  - type: switch
    id: escolher
    cases:
      - when: "true"
        flow:
          - type: step
            id: s1
            name: S1
            prompt: a
    default:
      - type: step
        id: s2
        name: S2
        prompt: b
  - type: loop
    id: repetir
    max_iterations: 2
    until: "outputs.repetir.iterations >= 2"
    flow:
      - type: step
        id: l1
        name: L1
        prompt: c
"#;
        let recipe = parse(yaml).expect("recipe");
        assert!(recipe.has_dag());
        assert_eq!(recipe.all_steps().len(), 3);
        assert!(matches!(recipe.flow[0], FlowItem::Switch(_)));
        assert!(matches!(recipe.flow[1], FlowItem::Loop(_)));
        let bad = yaml.replace("max_iterations: 2", "max_iterations: 0");
        assert!(parse(&bad).is_err());
    }
}

#[cfg(test)]
mod schema_tests {
    use super::parse;

    #[test]
    fn rejects_a_recipe_without_steps() {
        let error = parse("id: vazia\nname: Vazia\nflow: []\n").expect_err("empty flow");
        assert!(error.to_string().contains("ao menos um step"));
    }

    #[test]
    fn rejects_duplicate_step_ids() {
        let yaml = r#"
id: dup
name: Dup
flow:
  - type: step
    id: s1
    name: Um
    prompt: a
  - type: step
    id: s1
    name: Dois
    prompt: b
"#;
        let error = parse(yaml).expect_err("duplicate id");
        assert!(error.to_string().contains("duplicado") || error.to_string().contains("s1"));
    }

}
