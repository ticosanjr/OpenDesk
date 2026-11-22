# ADR 0016 — Runtime inventory do Studio e SSE sem timeout total

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** o Studio só lia arquivos, então skills/comandos/MCPs carregados
  pelo runtime do OpenCode (built-in/plugins/MCP) apareciam vazios. Runs longos
  eram marcados como FAILED exatamente aos 120s, mesmo depois de editar
  artefatos, por causa do timeout global do cliente HTTP.

## Decisão

- O Studio consulta o runtime do OpenCode:
  - `/api/skill`, `/api/command`, `/api/mcp`, `/api/agent`, `/api/model`;
  - itens runtime são exibidos como read-only, com contadores e status de MCP;
  - o Efetivo mescla runtime + global + workspace + ancestrais;
  - o workspace descobre `.opencode` e `opencode.json(c)` em todos os ancestrais.
- O cliente HTTP separa:
  - cliente normal com timeout de 120s para chamadas REST;
  - cliente de streaming sem timeout total, apenas `connect_timeout`, com
    `Accept: text/event-stream` e `Accept-Encoding: identity`.
- Se o stream cair, o runner:
  - tenta `wait_session`;
  - consulta `messages`;
  - considera sucesso se houver resposta ou artefatos.
- Daemons dedicados são marcados com `OPENDESK_DAEMON` e
  `OPENDESK_PARENT_PID`; no boot, órfãos do OpenDesk são encerrados.
- `scripts/verify-real.sh` executa um task real e exige SUCCESS, REVIEW e
  artefato; o gate pode rodá-lo com `OPENDESK_REAL_TASK=1`.

## Consequências

- Skills/comandos/MCPs que o OpenCode carrega aparecem no Studio mesmo sem
  arquivos em `~/.config/opencode`.
- Builds longos não são mais interrompidos pelo timeout do cliente.
- Estado do Kanban reflete o resultado real (REVIEW/build_complete).
- A verificação real consome tokens e é opt-in.
