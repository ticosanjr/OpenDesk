# ADR 0018 — Explorador de arquivos, feedback global e gestão de workspace

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** a página Arquivos ainda era um placeholder, ações destrutivas
  usavam `window.confirm`, erros e alertas de budget ficavam só em banners
  locais, o workspace não podia ser editado/removido pela UI e formulários de
  tarefa/chat/routine repetiam implementações de agente/modelo/prompt.

## Decisão

- O backend expõe `GET /api/files/tree` e `GET /api/files/content` limitados ao
  `root_path` do workspace:
  - caminhos relativos, sem `..`, barra invertida ou caminho absoluto;
  - `canonicalize` + verificação de prefixo para impedir escape por symlink;
  - `.git`, `node_modules`, `target`, `dist`, `.cache` e `.vite` são omitidos;
  - prévia textual limitada a 512 KB, com detecção de binário.
- A UI de Arquivos navega por diretórios, filtra, mostra breadcrumb, tamanho,
  modificação e prévia; abrir pasta/arquivo continua via plugin opener.
- Um `ToastProvider` global substitui avisos ad-hoc. Erros ficam até serem
  fechados; sucessos expiram; ações podem incluir “Abrir”, “Tentar de novo” etc.
- `ConfirmDialog` substitui `window.confirm` e recebe título, consequência e
  tom destrutivo.
- `ErrorBoundary` isola falhas de render por rota.
- `budgetAlert` e `RunFinished` passam a aparecer no shell; runs também disparam
  notificação nativa via `tauri-plugin-notification`.
- Workspaces ganham edição (nome, cor, política, timezone, e-mail padrão) e
  remoção apenas do registro; a pasta no disco nunca é apagada.
- O switcher de workspace vira popover com busca por nome/caminho, cor e ações.
- Agentes/modelos/prompts são componentes compartilhados:
  - `AgentCombobox` mescla global, workspace, runtime e built-ins e aceita ID
    manual;
  - `ModelCombobox` filtra, mostra health e aceita valor customizado;
  - `SlashTextarea` oferece `/` para comandos e skills do runtime.
- Criar e editar tarefa usam o mesmo formulário.
- Rotinas ganham duplicação, teste de notificação e histórico por `schedule_id`
  em `task_runs`.
- Rotas React são carregadas sob demanda (`React.lazy`) para reduzir o chunk
  inicial.
- O banco existente é copiado para `*.db.bak` (rotação de 3) antes de migrar.

## Consequências

- O usuário consegue inspecionar input/, output/ e .opencode/ sem sair do app.
- Ações destrutivas e alertas ficam consistentes e acessíveis.
- O mesmo agente/modelo/prompt funciona em Kanban, Chat e Rotinas.
- O histórico de rotina passa a ser auditável por run.
- O release inicial carrega menos JavaScript e o upgrade do banco tem rollback
  local.
