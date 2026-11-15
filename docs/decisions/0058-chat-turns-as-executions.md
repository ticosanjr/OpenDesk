# 0058 — Turnos de chat como execuções e observabilidade por origem

- **Status:** aceito
- **Data:** 2026-11-15
- **Contexto:** pedidos feitos no chat/Telegram não apareciam em Execuções, os
  artefatos criados por conversa não entravam em Artefatos e o timeout de cinco
  minutos interrompia trabalhos longos. Métricas separavam apenas tarefas.

## Decisão

1. **`chat_turns` registra todo turno.** A tabela guarda thread, canal, agente,
   modelo, status, tokens, custo, duração, conteúdo do usuário/assistente e
   erro. Turnos só de texto contam tokens e mensagens, mas não viram execução.
2. **Execução só quando há ferramenta ou arquivo.** Ao terminar o turno, o
   OpenDesk verifica chamadas de ferramenta e o diff da mensagem do usuário. Se
   houve uso de ferramenta, `chat_send_file` ou arquivo alterado, cria uma linha
   em `task_runs` com `execution_mode=CHAT`, `source=chat`, `channel` e
   `origin_ref=thread_id`. O turno guarda o `run_id`.
3. **Artefatos do chat.** Diffs e writes do turno são registrados em
   `artifacts_provenance` com o `run_id`; a lista e o modal derivam
   `source/channel/origin_ref` do run e exibem “chat · Telegram/Signal/Desktop”.
   Arquivos enviados por `chat_send_file` também são registrados.
4. **Sem timeout de cinco minutos.** O limite passou a 2 horas apenas como
   proteção; o usuário pode parar a qualquer momento. `POST
   /api/chat/threads/{id}/interrupt` cancela o token do turno e interrompe a
   sessão no OpenCode.
5. **Telegram concorrente.** O chat roda em background, então o long polling
   continua recebendo `/parar` e novas mensagens. Durante um turno, novas
   mensagens recebem “Ainda estou trabalhando”; a cada 2 minutos o bot envia um
   aviso de progresso.
6. **Observabilidade por origem.** `task_runs.source` é preenchido no dispatch
   (task/routine/recipe/automation) e o chat entra via `chat_turns`. `/api/metrics`
   passa a devolver execuções, mensagens, tokens, custo, sucesso e p95 por
   origem/canal; a página ganhou a tabela “Por origem”.
7. Migração 0040 adiciona as colunas e `chat_turns`; runs antigos são
   backfilled a partir da origem da tarefa/agenda.

## Consequências

- Conversas com arquivos/ferramentas aparecem em Execuções e em Artefatos com o
  canal de origem; conversas triviais continuam apenas no chat.
- O usuário vê “pensando”, tempo decorrido, estado e link da execução, e pode
  parar sem esperar cinco minutos.
- Métricas deixam de misturar trabalho, rotina, receita, automação e chat.
