# 0086 — Custo estimado para harnesses CLI

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** adapters CLI podem não emitir tokens/custo; o Budget Guard ficava cego.

## Decisão

1. Quando um run CLI termina sem `input_tokens`/`output_tokens`, estimar com
   `chars/4` sobre prompt e resumo.
2. Aplicar tabela de preços por família (DeepSeek/Flash, GLM, Claude, GPT,
   Gemini); modelos locais custam 0 e modelos desconhecidos ficam sem preço.
3. Preencher `RunUsage` e registrar `opendesk.usage.estimated` no log com
   `priced` para diferenciar estimativa de telemetria real.
4. Nunca sobrescrever usage real reportado pelo harness.

## Consequências

- Budget e observabilidade passam a ter sinal mesmo em CLIs sem telemetria.
- A precisão é aproximada e explicitamente marcada; um tokenizador real pode
  substituir a heurística sem mudar o contrato.
