# 0104 — Eval Gate histórico de skills

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o aprendizado promovia lições para propostas de skill sem um
  critério objetivo de regressão. O `skill_eval` só mostrava métricas passivas.

## Decisão

1. `skill_eval_gates` (migração 0056) registra uma avaliação por skill:
   status, menções, sucessos, falhas, taxa, thresholds, detalhes e decisão.
2. `POST /api/skills/{id}/gate` recebe `days`, `min_samples` e
   `min_success_rate`; calcula as métricas históricas via `skill_eval::evaluate`.
3. Status:
   - `INSUFFICIENT` quando amostras < `min_samples`;
   - `BLOCKED` quando a taxa fica abaixo do threshold;
   - `PASS` caso contrário.
4. `GET /api/skills/{id}/gate` devolve o último gate.
5. `POST /api/skills/{id}/gate/{gate_id}/decide` exige `approved`; aprovar um
   gate `BLOCKED`/`INSUFFICIENT` é conflito. A decisão humana fica registrada
   com nota e timestamp.
6. A avaliação é determinística e não chama modelo nem executa ferramentas; é a
   base histórica sobre a qual o replay sombra de casos vai rodar.

## Consequências

- Uma skill com regressão histórica não pode ser aprovada sem ajustar conteúdo
  ou thresholds.
- Amostra insuficiente falha fechado, evitando promoção por um único run.
- Casos curados, execução sombra sem efeitos e rollback de arquivo ainda faltam
  para fechar REQ-16.
