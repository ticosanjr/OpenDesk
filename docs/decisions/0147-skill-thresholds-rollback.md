# 0147 — G3: thresholds, promoção e rollback de skill

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** um replay sombra precisava bloquear regressões e permitir voltar
  a uma versão anterior sem edição manual.

## Decisão

1. `ShadowThresholds` define `min_success_rate` e `max_regression` (padrão 0.8 e
   0.1).
2. O run compara com o último run da mesma skill:
   - abaixo do mínimo → `BLOCKED`;
   - regressão acima do máximo → `BLOCKED`.
3. `POST /api/skills/{id}/shadow/{run_id}/promote` exige `approved: true` e um
   run `PASS`; grava `promoted_at`.
4. `POST /api/skills/{id}/rollback` restaura o backup mais recente do
   `SKILL.md` usando `file_ops::restore_backup`.
5. Testes cobrem regressão bloqueada, promoção sem aprovação, promoção de run
   bloqueado e rollback restaurando o conteúdo anterior.

## Consequências

- A aprovação é explícita e registrada no run promovido.
- O rollback usa os backups já criados por toda escrita do Studio; não há cópia
  paralela.
- A promoção não reescreve a skill: ela promove o resultado avaliado; o arquivo
  é alterado pelo Studio antes do replay.
