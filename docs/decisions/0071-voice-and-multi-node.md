# 0071 — Voz, multi-node e binários (onda 8)

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** a v1.0 precisa aceitar voz e escalar execução para mais de uma
  máquina sem quebrar a fila, o budget ou a observabilidade.

## Decisão

1. **Voz delegada ao host.** `voice_settings` guarda os comandos de STT/TTS e
   seus argumentos com placeholders (`{audio}`, `{out_base}`, `{out_audio}`,
   `{text}`, `{voice}`, `{language}`). O OpenDesk não baixa modelos.
   - STT padrão: `whisper-cli/whisper` com saída `.txt`.
   - TTS: `piper`, `espeak-ng`, `espeak` ou `say`, com argumentos pragmáticos.
   - `GET /api/voice/status` informa capacidades e o motivo quando falta algo.
2. **Áudio não fica no workspace por padrão.** O STT grava um temporário em
   `data/voice`, transcreve e apaga. O TTS grava em
   `output/YYYY-MM/voice/<id>.wav`, respeitando a regra determinística de
   entrega. Texto e áudio têm limite de tamanho.
3. **Nós puxam trabalho.** `workers` ganha um protocolo real:
   - `POST /api/nodes` registra e devolve um token (hash no banco);
   - `POST /api/nodes/heartbeat` mantém status/capabilities e `last_seen_at`;
   - `GET /api/nodes/next-job` reivindica atomicamente o job mais antigo,
     grava `assigned_node_id`, cria o run e devolve workspace root + run id;
   - heartbeat e complete do job exigem posse;
   - complete fecha fila, run, tarefa e publica eventos.
4. **Token separado do usuário.** O middleware aceita token de nó apenas nos
   endpoints do protocolo; listar/criar nós continua exigindo o token local ou
   sessão.
5. **Work stealing explícito.** O worker local e os nós disputam a mesma fila;
   quem ganhar o `UPDATE ... WHERE status='QUEUED'` executa. Isso evita um
   segundo scheduler e mantém uma única fonte de verdade.
6. **Binários do nó são declarados.** `capabilities` reporta harnesses e
   binários disponíveis; a UI mostra online/offline, endpoint e última visita.
   O workspace precisa estar montado no nó (NFS/rsync/SMB) porque o prompt e os
   artefatos vivem no disco.
7. **CLI.** `opendesk nodes list` para operação rápida.

## Consequências

- Voz funciona offline e sem vendor lock-in; a qualidade depende do modelo que o
  usuário já tem instalado.
- Multi-node é opt-in e auditável; o nó não recebe o token de usuário.
- A central não empurra comandos arbitrários: o worker puxa apenas jobs já
  validados e enfileirados.
- Latência e disponibilidade de workspace compartilhado são responsabilidade da
  infraestrutura do usuário, documentada na UI.
