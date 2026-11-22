# ADR 0001 — OpenCode V2 only

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** o documento de requisitos do OpenDesk foi escrito para a API e
  os formatos de configuração do OpenCode V1, mas o ambiente de desenvolvimento
  roda OpenCode 2.2.5.

## Decisão

O OpenDesk implementa exclusivamente o OpenCode V2. Não haverá branch de
compatibilidade V1 nem migração automática de configuração V1.

## Evidências que motivaram a decisão

- `opencode serve` no V2 expõe `/api/*` (não `/v1/*`) e exige HTTP Basic
  (`opencode:<senha impressa no boot>`). `/api/info` retorna 401 sem auth.
- `opencode agent list` não existe no V2; a validação de agentes usa
  `GET /api/agent`.
- `/api/model`, `/api/agent` e `/api/config` são location-scoped via
  `location[directory]` e exigem que o diretório seja um location reconhecido
  (por exemplo, com `.opencode/`).
- A configuração mudou: `permissions` (lista de regras), `commands` (plural) e
  `mcp.servers` com `disabled` e `type: "remote"`.
- Frontmatter de agente V2 não aceita `temperature`, `tools` nem `permission`;
  usa `permissions` e, quando aplicável, `steps`, `color` e `hidden`.
- `effort` não é um parâmetro de sessão. O V2 usa variantes (`provider/model#high`)
  e `Model.Ref.variant`.

## Consequências

- O adaptador `opencode_client` fala apenas `/api/*` e sempre envia Basic auth.
- O Studio grava formatos V2 e valida com `GET /api/agent`, não com CLI.
- A UI de effort resolve variantes quando existirem e mantém o nível como
  metadado de roteamento quando não existirem.
- A UI de temperatura de agente é removida; temperatura pode ser configurada
  por provider/model no OpenCode, não no arquivo do agente V2.
- Google Keep não é usado para contas comuns; o backlog usa Google Tasks.
- Uma eventual necessidade de V1 será um novo ADR e um adaptador separado, não
  um condicional espalhado pelo código.
