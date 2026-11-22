# 0136 — G1: casos curados de avaliação de skill

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** o Eval Gate histórico não tinha dataset curado.

## Decisão

1. Migração `0057` cria `skill_eval_cases`:
   - workspace, skill, nome, prompt;
   - `expected_contains` e/ou `expected_schema_json`.
2. CRUD no core (`skill_cases`): listar, criar, apagar, validar JSON Schema.
3. API:
   - `GET/POST /api/skills/{id}/cases`
   - `DELETE /api/skills/{id}/cases/{case_id}`
4. Teste cobre criação, listagem, exclusão e schema inválido.
5. Core soma 234 testes; shell 357 (591 total).

## Consequências

- O dataset curado está pronto para o replay sombra (G2).
- A UI de casos e o executor sombra continuam pendentes.
