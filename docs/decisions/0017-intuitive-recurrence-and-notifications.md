# ADR 0017 — Recorrência intuitiva, timezone local e notificações por novidade

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** a criação de rotinas exigia cron cru, o timezone padrão era UTC e
  não havia como notificar por Signal nem evitar notificações repetidas. O
  usuário também precisa de rotinas separadas por workspace e de editar uma
  rotina existente sem recriá-la.

## Decisão

- A UI oferece uma recorrência estruturada (`once`, `hourly`, `daily`, `weekly`,
  `weekdays`, `monthly`, `custom`) com calendário/horário e prévia dos próximos
  disparos. O cron é derivado no backend por
  `queue_scheduler::recurrence::Recurrence::to_cron`.
- O JSON original da recorrência é persistido em `schedules.recurrence_json`;
  `cron_expression` continua sendo o contrato do motor e é regenerado em
  create/update.
- `POST /api/schedules/preview` valida a recorrência, devolve o cron, a
  descrição humana e as 5 próximas ocorrências.
- O timezone padrão vem do sistema via `iana-time-zone`, permanece editável e
  cada rotina guarda o seu.
- Rotinas de uma vez (`once`) são desativadas depois de enfileirar; datas no
  passado são rejeitadas.
- Rotinas com `end_date` são desativadas quando expiram.
- Se o motor OpenCode estiver offline no slot, o disparo é marcado
  `SKIPPED_OFFLINE` e o próximo slot é preservado.
- `notify_signal` e `notify_signal_recipients` permitem entrega Signal; vazio usa
  a allowlist do Signal.
- `notify_only_on_change` compara o SHA-256 do artefato principal (ou do resumo)
  com `schedules.last_output_hash`; quando não muda, exportações continuam mas
  e-mail/Signal são omitidos.
- A rotina pertence ao workspace ativo; trocar de workspace é pré-requisito para
  gerenciar rotinas de outro workspace.
- O formulário é o mesmo para criar e editar; o agente é um combobox opcional
  com busca e a política do roteador é herdada do agente/workspace até ser
  alterada manualmente.

## Consequências

- O usuário não precisa conhecer cron; o modo custom continua disponível.
- Notificações repetidas deixam de poluir e-mail/Signal sem interromper a
  sincronização Google.
- O motor mantém compatibilidade com cron existente e com rotinas antigas sem
  `recurrence_json`.
- Testes cobrem tradução de recorrência, timezone, expiração, hash de novidade e
  os novos campos de persistência.
