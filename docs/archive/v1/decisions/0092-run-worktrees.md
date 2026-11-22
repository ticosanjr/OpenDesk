# 0092 — Worktrees isolados por run

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** runs editavam a árvore principal diretamente. Não havia como
  revisar, comparar ou descartar mudanças antes de aplicá-las, nem isolar A/B.

## Decisão

1. `worktree::prepare` cria isolamento por run:
   - repositório Git com `HEAD`: `git worktree add -b opendesk/wt-<id> <dir> HEAD`;
   - sem Git ou sem commit inicial: overlay por cópia, ignorando `.git`,
     `node_modules`, `target`, `output` e `.cache`.
2. O diretório fica em `<data_dir>/worktrees/<id>`, nunca dentro do workspace.
3. Metadados (`id`, workspace, run, modo, branch, commit base, status, merge)
   vivem em `run_worktrees` (migração 0052).
4. `worktree::diff`:
   - Git: `git add -N .` + `git diff` para incluir arquivos novos;
   - overlay: comparação byte a byte com a árvore principal (`A`/`M`).
5. `worktree::merge` exige `approved=true` na API/tool. No modo Git, as
   alterações do worktree são commitadas no branch temporário e mescladas com
   `--no-ff`; conflito aborta e devolve `Conflict` sem tocar na árvore. No modo
   overlay, apenas arquivos novos/alterados são copiados.
6. `cleanup` remove worktree, branch temporário ou diretório overlay.
7. API: `GET/POST /api/worktrees`, `GET /api/worktrees/{id}/diff`,
   `POST /api/worktrees/{id}/merge`, `DELETE /api/worktrees/{id}`.
8. MCP de controle expõe `worktree_prepare`, `worktree_list`,
   `worktree_diff`, `worktree_merge` e `worktree_cleanup`; merge continua
   exigindo `approved=true`.

## Consequências

- Nenhuma alteração chega à árvore principal sem diff + aprovação.
- Conflitos são explícitos e reversíveis.
- O executor de runs ainda não cria worktrees automaticamente; esta fatia
  entrega a infraestrutura, a API e as tools. O wiring no `run_execution` e o
  A/B ficam para a próxima fatia de REQ-06.
- Overlay não detecta deleções (proposital: nunca apaga arquivo do usuário sem
  confirmação).
