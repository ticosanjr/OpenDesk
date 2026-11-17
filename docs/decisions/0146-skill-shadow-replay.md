# 0146 — G2: replay sombra de skills

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** o Eval Gate histórico media menções em runs reais; faltava
  executar os casos curados em um ambiente sem efeitos.

## Decisão

1. Migração `0059` cria `skill_shadow_runs` e `skill_shadow_results`.
2. O core executa os casos via trait `ShadowExecutor`; o shell injeta
   `ShellShadowExecutor`.
3. O executor usa `generate_text` (uma chamada sem sessão, sem histórico e sem
   ferramentas) com um prompt que inclui a skill e o caso.
4. Cada resultado valida:
   - `expected_contains`;
   - `expected_schema_json` via JSON Schema;
   - tokens estimados, custo estimado e latência.
5. O run calcula sucesso, falha, taxa, regressão contra o run anterior e status
   `PASS`/`BLOCKED`.
6. Teste com arquivo-sentinela prova que o replay não altera o workspace.
7. API:
   - `GET/POST /api/skills/{id}/shadow`;
   - `GET /api/skills/{id}/shadow/{run_id}`.

## Consequências

- O replay não executa ferramentas nem sessões; não escreve no workspace.
- A qualidade depende do modelo escolhido e do prompt, mas o contrato é
  determinístico (contains/schema).
- Casos sem dataset retornam erro acionável.
