# 0224 — Editor e contrato visual de receitas DAG 2.0

- **Status:** aceito
- **Data:** 2026-11-23
- **Contexto:** o crate `opendesk-recipes` já executa `step`, `hitl`, `sink`,
  `branch`, `parallel`, `map`, `recipe_call`, `loop` e `switch`, com
  `input_schema`/`output_schema` e versionamento. O frontend ainda trata o fluxo
  como uma lista linear de `step | hitl | sink`, então uma receita avançada
  editada visualmente perde nós e campos.

## Decisão

1. `FlowNodeType` cobre os **nove** tipos do backend. `loop` e `switch` também
   são editáveis; não há conversão destrutiva para um subconjunto.
2. O editor visual é uma árvore/colunas em React + primitivos existentes, sem
   ReactFlow ou canvas pesado. Grupos (`parallel`, `map`, `branch`, `loop`,
   `switch`) renderizam filhos recursivamente.
3. O round-trip YAML ↔ visual preserva campos desconhecidos. A validação de
   sintaxe, ciclos, referências, schemas e `vault_refs` é feita pelo backend em
   `POST /api/recipes/validate`, que vira a única fonte de verdade.
4. `input_schema`/`output_schema` ganham um construtor simples de campos e uma
   aba de JSON bruto. `on_schema_error` aceita `retry_format` (máx. 2), `fail`
   e `hitl`.
5. `POST /api/recipe-runs/{id}/resume` continua runs `FAILED`, `TIMED_OUT` ou
   `CANCELLED`, preservando nós `DONE`; `POST .../nodes/{path}/retry` reexecuta
   um nó e seus descendentes. `COMPLETED`/`RUNNING_*` retornam `409`.
6. O histórico de versões existente ganha uma gaveta com diff e rollback; o
   detalhe do run mostra estado por nó, incluindo itens de `map` e ramos de
   `parallel`.
7. Os componentes novos respeitam o limite de 500 linhas; `RecipesPage` e
   `RecipeRunDetailPage` são divididos antes de receber o editor.

## Consequências

- Uma receita criada na UI, no YAML ou por IA tem o mesmo contrato.
- Nenhum nó ou campo é apagado ao alternar entre as abas.
- `map` vazio conclui o nó como `SKIPPED` em vez de falhar o DAG.
- O MCP expõe `recipe_validate` e usa o endpoint de resume correto.
