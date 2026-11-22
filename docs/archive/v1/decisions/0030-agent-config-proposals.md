# ADR 0030 — Auto-organização de agentes com aprovação HITL

- **Status:** aceito
- **Data:** 2026-11-07

## Decisão

- Agentes podem propor criação/edição/remoção de agentes, skills, comandos,
  receitas, rotinas, MCPs e arquivos dentro de `.opencode`.
- Toda proposta entra em `config_proposals` com status `PENDING` e **não é
  aplicada** até aprovação explícita no Attention Card.
- O MCP `opendesk-config` expõe ferramentas de proposta; ele usa a API local
  quando disponível e cai para SQLite caso o app esteja fechado.
- A aplicação reutiliza `config_sync` (agents/skills/commands/jsonc), receitas e
  schedules; não duplica escrita de arquivos.
- Propostas aprovadas viram `APPLIED`; falhas viram `FAILED` com erro visível.
- Workspaces reais ficam em `~/Documents/OpenDesk Workareas` (ou na raiz
  configurada) e não em `/tmp`. O ambiente de teste pode usar
  `OPENDESK_ALLOW_TEMP_WORKSPACES=1`; workspaces temporários de produção são
  recusados.

## Consequências

- Agentes conseguem criar estruturas e fluxos sob medida sem alterar o
  workspace sem supervisão.
- Toda mudança sensível tem botão Aprovar/Negar e trilha de auditoria.
- O mesmo mecanismo cobre novas categorias de configuração via `kind`.
