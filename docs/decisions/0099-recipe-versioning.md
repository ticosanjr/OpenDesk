# 0099 — Versionamento, diff e rollback de receitas

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** editar uma receita sobrescrevia o YAML anterior. Não havia como
  comparar versões nem voltar atrás sem backup manual.

## Decisão

1. Todo `save_yaml` grava uma versão em `recipe_versions` (migração 0054) quando
   o hash do conteúdo muda; conteúdo idêntico não duplica.
2. A versão guarda conteúdo completo, SHA-256 e timestamp; a primeira gravação
   de uma receita nova registra o YAML já com `enabled: false`.
3. API:
   - `GET /api/recipes/{id}/versions`
   - `GET /api/recipes/{id}/versions/{version_id}`
   - `GET /api/recipes/{id}/versions/{version_id}/diff?against=current|<id>`
   - `POST /api/recipes/{id}/rollback` com `version_id`.
4. Diff usa `similar::TextDiff` com 3 linhas de contexto; o destino padrão é o
   arquivo atual.
5. Rollback reescreve o conteúdo pela mesma `save_yaml`, portanto revalida o
   parser, atualiza o índice, o watcher e o trigger source, e registra uma nova
   versão em vez de apagar histórico.
6. O MCP de controle expõe `recipe_versions`, `recipe_diff` e
   `recipe_rollback`, com a mesma validação da API.
7. Versões vivem no SQLite (estado operacional), enquanto o YAML atual continua
   sendo a SSOT no workspace.

## Consequências

- O usuário pode comparar e reverter uma receita sem sair do OpenDesk.
- O histórico cresce com edições reais; conteúdo repetido não infla a tabela.
- Orçamento isolado por sub-receita continua pendente em REQ-04; a UI de
  histórico/rollback também pode ser adicionada em uma fatia de produto.
