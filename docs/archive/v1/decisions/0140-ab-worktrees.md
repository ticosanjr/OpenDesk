# 0140 — C2: experimentos A/B com worktrees

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** o isolamento por run já existia, mas não havia como comparar
  dois agentes/modelos no mesmo prompt antes de aplicar o resultado.

## Decisão

1. Migração `0058` cria `run_ab_groups` (workspace, título, prompt, status,
   vencedor e timestamps).
2. `POST /api/runs/ab`:
   - cria o grupo;
   - cria duas tarefas `AB_TEST` com tags `ab:<group>` e `ab-slot:<a|b>`;
   - enfileira os dois jobs com o mesmo prompt e os modelos/agentes escolhidos.
3. A fila força worktree para tarefas com tag `ab:` mesmo que o setting global
   `worktree_runs` esteja desligado.
4. `GET /api/runs/ab/{id}` compara os dois slots:
   - status, agente/modelo;
   - custo, tokens e duração;
   - diff e estatísticas por arquivo.
5. `POST /api/runs/ab/{id}/choose` exige `approved: true`, mescla apenas o
   worktree vencedor, descarta o perdedor e grava o vencedor no grupo.
6. A página de worktrees ganhou a aba A/B com criação, lista e comparação.
7. Corrigido um bug de colisão de branch: `worktree::prepare` usava os 8
   primeiros caracteres do UUID v7, que são compartilhados por worktrees criadas
   no mesmo milissegundo. Agora usa o último segmento aleatório.

## Consequências

- O A/B usa o mesmo executor e o mesmo isolamento dos runs normais.
- A escolha é a aprovação HITL; não há merge automático.
- O teste Rust usa repositório Git temporário e prova que só o vencedor é
  mesclado; o E2E cobre a criação e a abertura dos dois slots sem daemon.
