# 0109 — Worktrees opt-in no executor de runs

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o gerenciador de worktrees existia, mas o executor sempre rodava
  na árvore principal. O REQ-06 pedia isolamento real no run.

## Decisão

1. Setting `worktree_runs` (bool, default `false`) liga o isolamento por run.
2. Em `queue_manager::execute_job`, quando ligado:
   - cria worktree Git/overlay **antes** de planejar output/run;
   - planeja o run com o root do worktree, então output e artefatos nascem
     isolados;
   - persiste `run_worktrees` com o `run_id`;
   - executa o harness com `workspace_root = worktree`;
   - registra artefatos relativos ao worktree.
3. A árvore principal não é tocada. Merge/descarte continuam via
   `/api/worktrees/{id}/merge` ou tools MCP `worktree_merge`, com aprovação.
4. Falha ao preparar o worktree falha o job com mensagem explícita; não há
   fallback silencioso para a árvore principal.
5. O comportamento padrão permanece idêntico: sem o setting, o run usa o
   workspace real.
6. Output/artefatos ficam no worktree até o merge; a UI de worktrees e o A/B
   continuam pendentes.

## Consequências

- É possível operar com isolamento real assim que o setting é habilitado.
- Runs isolados não aparecem automaticamente na árvore principal; o merge é uma
  etapa explícita.
- Limpeza de worktrees órfãs em caso de falha antes da criação do run ainda
  precisa de um sweeper de boot.
