# Contrato OpenCode V2

O OpenDesk fala com o daemon via `/api/*` e SSE em `/api/event`. Não usa `/v1/*`.

## Sessões

| Uso | Sessão |
|---|---|
| Tarefa nova | `POST /api/session`, `session_id` novo |
| Follow-up de tarefa | reutiliza `task_runs.session_id` mais recente |
| Chat contínuo | reutiliza `chat_threads.opencode_session_id` |
| Fork/swarm | sessão nova por card |
| Retry com outro modelo | sessão nova ou continuação explícita |

## Envelope SSE

```json
{
  "created": 1794056408170,
  "type": "session.text.delta",
  "data": { "delta": "texto" },
  "location": { "directory": "/workspace" },
  "durable": { "aggregateID": "ses_...", "seq": 1 }
}
```

## Eventos usados

| Evento | Efeito no OpenDesk |
|---|---|
| `session.execution.started` | status `RUNNING` |
| `session.step.started/ended` | passo, tokens, custo |
| `session.reasoning.started/delta/ended` | bloco de raciocínio |
| `session.text.started/delta/ended` | comentário e resultado |
| `session.tool.input.started/ended` | argumentos da ferramenta |
| `session.tool.called` | ferramenta em execução |
| `session.tool.success/failed` | resultado da ferramenta |
| `session.usage.updated` | telemetria de tokens |
| `permission.asked` | permissão interativa |
| `session.execution.succeeded/failed` | fim terminal |
| `session.idle` | ocioso |

Eventos desconhecidos são preservados no JSONL e ignorados pelo parser, sem
interromper o run.

## Permissão

```http
POST /api/session/{session_id}/permission/{request_id}/reply
Content-Type: application/json

{ "decision": "once" | "always" | "reject" }
```

Timeout configurável em `permission_timeout_minutes` (default 10). No timeout o
OpenDesk envia `reject`, interrompe a sessão, marca o run como falho e move a
tarefa para `BLOCKED`.

## Fixtures

Os formatos acima são cobertos pelos logs reais em `runs/*.jsonl` e pelos testes
do `run_timeline`.
