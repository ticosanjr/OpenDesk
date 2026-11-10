# 0044 — MCP de controle operacional e criação desligada por padrão

- **Status:** aceito
- **Data:** 2026-11-10
- **Contexto:** agentes já podiam propor mudanças de configuração pelo MCP
  `opendesk-config`, mas não tinham uma forma segura de operar tarefas,
  execuções, rotinas, receitas, fila e artefatos. Além disso, rotinas e receitas
  podiam nascer ativas, permitindo trabalho automático antes de revisão humana.

## Decisão

1. Um novo MCP local `opendesk-control` (stdio, subcomando `mcp-control`) fala
   com a API local do OpenDesk usando `api.json` + bearer token. Ele não duplica
   execução: apenas valida e opera o estado no backend.
2. O servidor expõe tarefas, execuções, rotinas, receitas, fila, artefatos e
   leitura da atenção. HITL, permissões e propostas de configuração são
   **somente leitura**; a decisão continua humana.
3. Mudanças estruturais em agentes, skills, comandos, MCPs e `opencode.jsonc`
   continuam passando pelo `opendesk-config` como propostas com aprovação.
4. Todo workspace recebe `opendesk-config` e `opendesk-control` no
   `opencode.jsonc`: na criação e, para workspaces existentes, no boot via
   `proposals::ensure_all`.
5. Um pacote base `controle-opendesk` (skill + comandos) é instalado
   automaticamente em **novos** workspaces: criação manual, template e fábrica
   padrão. A instalação usa `skip_conflicts`, registra proveniência e nunca
   sobrescreve arquivos existentes. Workspaces existentes recebem o MCP, mas não
   o conteúdo do pacote automaticamente.
6. Toda rotina e receita nova nasce **desligada**:
   - `schedules::create` grava `is_active = false`; ativar exige
     `POST /api/schedules/{id}/active`.
   - `recipes::store::save_yaml` grava `enabled: false` quando o arquivo é novo;
     edições preservam o estado e `POST /api/recipes/{id}/enabled` liga.
   - pacotes normalizam receitas para `enabled: false` no planejamento.
   - a UI (formulário de rotina e template de receita) reflete o padrão.

## Consequências

- Agentes ganham autonomia operacional com trilha de auditoria na API local.
- O MCP de controle só funciona com o app/headless em execução; se a API não
  estiver disponível, a ferramenta devolve erro explicativo.
- É uma exceção deliberada ao ADR 0032 (“nada é instalado automaticamente”):
  apenas o pacote base operacional é automático; os demais continuam opcionais.
- Rotinas/receitas novas não disparam até ativação explícita, reduzindo risco de
  custo e efeitos externos não revisados.
- Testes cobrem bundle, criação de workspace, MCPs, defaults desligados e a UI.
