# ADR 0015 — Importação fiel, retry durável e rollback

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** a importação do Studio tratava Markdown como texto puro, perdendo
  frontmatter e assets; entregas falhas não sobreviviam a restart; não havia
  rollback pela UI.

## Decisão

- Importação de agente/comando/skill faz parse do frontmatter e grava os campos
  estruturados (`name`, `model`, `mode`, `permissions`, `description`,
  `subagent`, etc.). O corpo vira system prompt/template/body.
- Skills importadas de ZIP/pasta preservam todos os arquivos da pasta do
  `SKILL.md` como `files`.
- GitHub descobre o default branch via API, aceita `@ref` e usa token
  (`GITHUB_TOKEN`/`GH_TOKEN`/`gh auth token`) para repositórios privados.
- Entregas SMTP falhas guardam payload em SQLite, `next_retry_at` e
  `max_attempts`; um worker tenta novamente após restart com backoff.
- Rollback:
  - OpenDesk: `.bak` do binário/AppImage é restaurado pelo helper e o app relança;
  - OpenCode: o binário anterior é salvo em `<opencode>.bak` antes do upgrade e
    pode ser revertido pela UI.

## Consequências

- Importar agentes de fontes comuns mantém permissões e modelo.
- Entregas não são perdidas quando o processo fecha.
- O usuário tem uma saída se uma atualização quebrar o app ou o motor.
- Assinatura do manifesto e retry durável de Google Docs/Drive continuam
  pendentes.
