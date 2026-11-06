# ADR 0002 — Metadados de apresentação de agentes

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** o usuário pediu identificação visual de agentes por avatar, cor
  e nome. O OpenCode V2 expõe `color` no frontmatter do agente, mas ignora
  `name` e campos desconhecidos; o `name` retornado por `/api/agent` é derivado
  do ID do arquivo.

## Decisão

O OpenDesk grava `name` e `avatar` no frontmatter YAML do próprio agente, ao lado
de `color`, que é nativo do OpenCode.

- `color`: campo V2 nativo (`#rrggbb`).
- `name`: rótulo de exibição usado pela UI do OpenDesk.
- `avatar`: emoji ou até dois caracteres exibidos em círculo colorido.

## Consequências

- A configuração continua em um único arquivo `.md` por agente, sem sidecar e
  sem espelho no SQLite.
- O OpenCode ignora `name`/`avatar` silenciosamente; o comportamento de runtime
  não muda.
- Outras ferramentas que leem o frontmatter podem ignorar os campos; eles não
  são uma API pública do OpenCode.
- Se o V2 passar a rejeitar campos desconhecidos no frontmatter, migraremos
  `name`/`avatar` para um sidecar `.opencode/opendesk/agents.json`; a UI já
  consome os campos de forma abstrata.
- A cor é lida tanto do arquivo quanto de `/api/agent`, mantendo consistência se
  o agente for editado em outro editor.
