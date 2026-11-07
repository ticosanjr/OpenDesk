# ADR 0022 — Model refs sem null e notificações sem repetição

- **Status:** aceito
- **Data:** 2026-11-07
- **Contexto:** sessões sem variante quebravam com
  `Expected string at ["model"]["variant"]`, pois `variant: Option<String>`
  virava `variant: null`. Além disso, fechar a janela apenas escondia o app na
  bandeja, mantendo fila/notificações ativos, e tarefas com Plan/Build/Audit
  geravam várias notificações para uma única execução percebida.

## Decisão

### Model refs
- `ModelRefDto::parse` é o parser único:
  - `provider/model`;
  - `provider/model#variant`;
  - variante vazia é descartada;
  - provider/id vazios ou ausentes são rejeitados.
- `ModelRefDto::to_payload` nunca inclui `variant` nulo/vazio.
- `create_session` omite `model` quando não há modelo válido e omite `variant`
  quando ausente.
- APIs validam modelos em tarefas, chat, dispatch, mensagens, agentes, comandos
  e regras do roteador antes de persistir.
- O roteador nunca monta `model#` vazio.

### Notificações
- Fechar a janela encerra o processo; não fica mais escondido na bandeja
  notificando sem o usuário saber.
- Tarefas com múltiplas fases notificam apenas no status terminal:
  `DONE`, `BLOCKED` ou `CANCELLED`.
- Runs sem task (rotinas) notificam uma vez por run.
- Artefatos do mesmo run geram no máximo um aviso.
- Falhas de rotina só notificam quando `last_run_at` muda.
- `markNotificationOnce` mantém LRU de chaves para bloquear duplicatas.
- Notificação nativa só é enviada quando a janela está oculta/sem foco; caso
  contrário apenas o toast é exibido.
- Permissão nativa é solicitada uma única vez.
- Toasts são limpos ao esconder a janela ou descarregar a página.

## Consequências

- Nenhum payload de sessão envia `variant: null` ou `model: null`.
- Modelos sem variante funcionam em tarefas, chat, rotinas e fallbacks.
- Fechar o app realmente para as notificações.
- Uma tarefa produz no máximo uma notificação terminal.
- Rotinas antigas não repetem alertas a cada tick.
