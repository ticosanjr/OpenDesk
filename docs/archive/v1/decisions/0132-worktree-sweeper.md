# 0132 — C3: sweeper de worktrees órfãs

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** runs isolados podiam deixar diretórios em `<data>/worktrees`
  após falhas antes da criação do run ou perda do registro.

## Decisão

1. `db::worktrees::sweep` roda no boot:
   - registros ACTIVE com path inexistente viram `REMOVED`;
   - diretórios em `<data>/worktrees` sem registro são apagados.
2. `AppState::initialize` chama o sweeper e apenas loga falhas.
3. O teste cobre registro órfão e diretório sem registro.

## Consequências

- Diretórios órfãos não acumulam indefinidamente.
- Worktrees ACTIVE válidas (aguardando merge) são preservadas.
- A UI/A-B continuam pendentes em C1/C2.
