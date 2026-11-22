# 0159 — A4/A5: portas da fila e scheduler no core

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** a fila ainda vivia no shell e chamava `learning`, `delivery`,
  `io_bus` e `api::tasks` diretamente.

## Decisão

1. `opendesk_core::queue_scheduler` passa a conter:
   - `queue_manager`, `run_execution`, `settlement`, `recurrence`, `cron_engine`.
2. O core expõe duas portas:
   - `QueuePorts::after_run` para learning/delivery/sinks;
   - `TaskDispatcher::dispatch_task` para autostart de tarefas.
3. O shell implementa `ShellQueuePorts` e `ShellTaskDispatcher` em
   `shell_queue_ports.rs`.
4. `CronEngine` guarda um `RecipeContext` tardio (`attach_context`) e o
   dispatcher; não depende mais de `Weak<AppState>`.
5. `QueueManager` recebe `notifier: Arc<dyn RunNotifier>` e não instancia mais
   `ShellRunNotifier` diretamente.
6. `tauri::async_runtime::spawn` foi trocado por `tokio::spawn`.
7. Os testes de fila/cron foram movidos para o core; `TestContext` ganhou um
   `EventBus`.

## Consequências

- A fila não conhece canais, Google, io_bus nem API HTTP.
- O shell faz o wiring tardio com `AppState::attach_runtime_ports()`.
- Testes: 25 no core para queue/cron; shell ficou com 336 testes e o core com
  284 (620 no total).
