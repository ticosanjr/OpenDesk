---
description: Cria ou revisa uma receita YAML do OpenDesk.
agent: build
---
Trabalhe a receita descrita abaixo.

1. Se já existir, leia com `recipe_list` e preserve fluxo, HITL e versões.
2. Se for nova, escreva YAML válido com `id`, `name`, `enabled: false`,
   `triggers` e `flow`.
3. Use o DAG completo quando fizer sentido:
   - `step`: executa um agente; aceita `when`, `input_schema`, `output_schema`,
     `on_schema_error` (`retry_format`, `fail` ou `hitl`), `retry_max` e
     `timeout_secs`.
   - `hitl`: checkpoint humano com `interaction_type`, `channels`,
     `timeout_minutes` e `on_timeout`.
   - `sink`: entrega o resultado (DOC_COMPILER, EMAIL_SMTP, TELEGRAM_MSG,
     SIGNAL_MSG, HTTP_WEBHOOK, WHATSAPP_MSG, WEBPUSH, SQL_INSERT, GIT_COMMIT).
   - `branch`: `when` + `then` + `else`.
   - `parallel`: ramos concorrentes com `join: all|any`.
   - `map`: fan-out sobre um array (`items`, `item_var`, `max_concurrency`).
   - `recipe_call`: sub-receita com `recipe`, `with` e `budget_usd`.
   - `loop`: iteração limitada por `max_iterations` e `until`.
   - `switch`: múltiplos casos com `default`.
4. Credenciais nunca entram no YAML. Declare `vault_refs` no step e use
   `{{vault:<id>}}` apenas em bordas HTTP/MCP; login no navegador é HITL.
5. Valide com `recipe_run` e `dry_run=true` antes de ligar.
6. Só ligue com `recipe_set_enabled` após revisão humana explícita.

Exemplo de contrato:

```yaml
id: exemplo-dag
name: Exemplo DAG
enabled: false
triggers: []
flow:
  - type: step
    id: coletar
    name: Coletar
    mode: build
    prompt: |
      Leia {{run.output_dir}}/input e devolva JSON.
    input_schema:
      type: object
      required: [url]
      properties:
        url: { type: string }
    output_schema:
      type: object
      required: [items]
      properties:
        items: { type: array, items: { type: string } }
    on_schema_error: retry_format
    retry_max: 2
    vault_refs: [stripe-live]
  - type: branch
    id: decidir
    when: "{{steps.coletar.structured.items}} != []"
    then:
      - type: map
        id: processar
        items: "{{steps.coletar.structured.items}}"
        item_var: item
        max_concurrency: 3
        flow:
          - type: step
            id: resumir
            name: Resumir item
            mode: build
            prompt: "Resuma {{items.item}} em uma linha."
    else:
      - type: hitl
        id: revisar-vazio
        interaction_type: BINARY_GATE
        channels: [DESKTOP_UI]
        timeout_minutes: 60
        on_timeout: PAUSE_AND_ALERT
        prompt_template: "A coleta veio vazia. Continuar?"
  - type: sink
    type: HTTP_WEBHOOK
    config:
      url: "https://exemplo.com/hook"
      method: POST
```

Nunca remova um checkpoint HITL para “simplificar” a receita e nunca escreva
segredo em texto puro no YAML.

Pedido: $ARGUMENTS
