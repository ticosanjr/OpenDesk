# 0075 — Worker remoto e distribuição do opendeskd (onda 8, fechamento)

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o protocolo multi-node existia (registro, claim, heartbeat,
  complete), mas nenhum cliente o consumia e o release não empacotava o binário
  `opendeskd`. Na prática, a onda 8 não era utilizável ponta a ponta.

## Decisão

1. **Worker no CLI.** `opendesk node run --central <url> --token <token>`
   registra capacidades, faz heartbeat, puxa jobs, executa e conclui. `--once`
   processa um job; `--poll` ajusta a espera.
2. **Apenas harnesses CLI.** O worker suporta `claude-code`, `codex`, `pi` e
   `hermes`, com os mesmos argumentos do adapter local. Jobs `opencode` falham
   com mensagem clara: executar OpenCode exige o daemon local, não o worker.
3. **Workspace compartilhado.** O worker usa o `workspace_root` devolvido pelo
   claim e recusa se a pasta não existir; a montagem (NFS/rsync/SMB) é
   responsabilidade da infraestrutura.
4. **Heartbeat durante a execução.** Um task em background envia heartbeat a
   cada 20 s; ao terminar, o worker envia `complete` com status, duração,
   `run_id` e resumo limitado. Texto do prompt nunca é logado.
5. **Token fora do argv quando possível.** `--token` é aceito, mas
   `OPENDESK_NODE_TOKEN` é a forma recomendada.
6. **`opendeskd` no release.** O script constrói e publica
   `opendeskd-<plataforma>` junto com o desktop. O updater do desktop ignora o
   asset extra (seleciona pelo nome portátil exato), então não há risco de
   atualização cruzada.

## Consequências

- Multi-node passa a funcionar de verdade: a central agenda e o worker executa.
- O usuário instala o daemon e o worker a partir dos artefatos oficiais, sem
  compilar.
- Jobs OpenCode continuam centralizados no daemon; a arquitetura não finge que
  um CLI remoto executa o protocolo do OpenCode.
