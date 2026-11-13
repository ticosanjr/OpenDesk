# Cobertura Rust — rumo a 80%

O gate usa dois limites de linhas (llvm-cov):

| Medida | Antes da onda 14 | Agora | Meta |
|---|---:|---:|---:|
| Total | 62,50% | **64,81%** | 80% |
| Core (db, api, recipes, fila, io_bus, dsh_router, …) | 68,67% | **71,14%** | 80% |

Comandos:

```bash
npm run gate:fast -- --rust     # inclui clippy e testes
scripts/gate.sh                 # inclui cobertura + E2E
cd src-tauri && cargo llvm-cov --summary-only > /tmp/cov.txt
scripts/coverage-core.sh /tmp/cov.txt
```

## Regra do ratchet

- `scripts/gate.sh` falha abaixo de **64,5%** totais e **70,8%** core.
- Ao cobrir um módulo, suba o limite até ~0,5 ponto abaixo do medido.
- Nunca baixe o limite para “passar”; prefira marcar o teste como opt-in.

## Próximos alvos (maiores lacunas)

| Arquivo | Linhas descobertas | Como cobrir |
|---|---:|---|
| `queue_scheduler/queue_manager.rs` | ~500 | worker/execução com daemon falso, lease e recuperação |
| `api/studio_ops_install.rs` | ~400 | instalação por git/npm e conflitos de destino |
| `api/tasks.rs` | ~400 | demais transições, dispatch e aprovação |
| `recipes/engine.rs` | ~300 | execução de passos com sinks e HITL |
| `queue_scheduler/cron_engine.rs` | ~300 | misfire, safe mode e dry-run |
| `io_bus/inbound/chat_bridge.rs` | ~300 | Telegram/Matrix com servidor HTTP falso |
| `api/schedules.rs` | ~280 | recorrência, timezone e edição |
| `api/engine.rs` | ~270 | logs, tail e reinício com daemon falso |

## Estratégia

1. Módulos puros primeiro (`parse_*`, payloads, políticas) — baratos e estáveis.
2. Handlers via `api::tests` e `test_support::context()` (rota + SQLite real).
3. Ponte externa com servidor local (wiremock/axum) para Google, SMTP, Telegram,
   Matrix e OpenCode; nada de rede real no gate.
4. Contratos reais (Google/conta dedicada, OpenCode) continuam opt-in e não
   entram no percentual.

Enquanto o core não chega a 80%, o gate protege o patamar atual para que novos
módulos não regridam a cobertura.
