# 0139 — C1: UI de worktrees

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** a API de worktrees existia desde a REQ-06, mas não havia como
  listar, revisar o diff, mesclar ou descartar pela UI.

## Decisão

1. `GET /api/worktrees` passa a devolver `WorktreeEntry`:
   - os campos do worktree;
   - `task_title` e `run_status` via join com `task_runs`/`tasks`;
   - `diff_stat` (arquivos, inserções, deleções) calculado só para worktrees
     ativas.
2. Nova página `/#/worktrees` no menu Operação:
   - lista worktrees com tarefa, modo, branch, diff, status e data;
   - modal de diff reutilizando `DiffView`;
   - merge com confirmação e `approved: true`;
   - descarte com confirmação destrutiva;
   - aba A/B (ver ADR 0140).
3. `RunDetailPage` mostra o badge “worktree isolado” com link para a página.
4. E2E com repositório Git temporário cobre listar, abrir diff, aprovar merge e
   verificar o arquivo no workspace principal.
5. O golden da rota nova foi adicionado; snapshots afetados pelo item extra na
   sidebar foram regravados.

## Consequências

- O fluxo de isolamento fica operável sem CLI/MCP.
- Descartar remove a cópia e a branch temporária; merge continua exigindo
  aprovação explícita.
- A lista calcula diff no handler; com muitas worktrees ativas isso pode ficar
  caro e deve migrar para background em uma próxima fatia.
