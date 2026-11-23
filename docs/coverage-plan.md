# Cobertura Rust — rumo a 80%

O gate usa dois limites de linhas (llvm-cov):

| Medida | Antes da 1.4.2 | Agora | Meta |
|---|---:|---:|---:|
| Total (`--workspace`) | 54,35% | **64,60%** | piso 63,0% |
| Negócio (domain, storage, engine, recipes, api, fila, io_bus, …) | 64,09% | **76,17%** | piso 75,5% (meta 75% atingida) |

Comandos:

```bash
npm run gate:fast -- --rust     # inclui clippy e testes
scripts/gate.sh                 # inclui cobertura + E2E
cd src-tauri && cargo llvm-cov --workspace --summary-only > /tmp/cov.txt
scripts/coverage-core.sh /tmp/cov.txt
```

## Regra do ratchet

- `scripts/gate.sh` falha abaixo de **63,0%** totais e **75,5%** no escopo de
  negócio (rebaseline de 2026-11-22 na extinção do core: fila e receitas agora
  contam em `engine`/`recipes`, o que reduziu a média sem regressão de código).
- Medição de 2026-11-23 após a Fase 7: **64,60%** total e **76,17%** de
  negócio, com a lacuna fechada por testes dos módulos novos (Vault/Bitwarden,
  poller/webhook, Discord/Slack/QR, doctor live e canais).
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

O escopo de negócio está em 76,17%, acima da meta de 75%. O total do workspace
inclui CLI, tray, updater, doctor, integrações de rede e o registro de handlers
MCP; o piso de 63,0% protege esse patamar. `engine`+`recipes` isolados ficam em
68,0%: a próxima onda deve priorizar worker/lease, cron e DAG para levar esse
recorte a 75%.

## Decisão de escopo (2026-11-17)

O usuário aceitou não bloquear o release final na meta de 80%. O gate continua
com o ratchet atual e nenhum limite será baixado para "passar". O piso final
será fixado no início da fase H2, com base na cobertura medida depois de A–G;
H4 não exige 80% se o piso acordado estiver verde e documentado.
