# 0031 — Fluxo unificado de receitas e rotinas acionadas por qualquer entrada

- **Status:** aceito
- **Data:** 2026-11-08
- **Contexto:** receitas só aceitavam um gatilho, `steps` + `hitl_checkpoints` +
  `sinks` separados e não podiam ser disparadas por rotinas. Rotinas só
  suportavam CRON e só executavam prompt.
- **Decisão:**
  1. O YAML canônico de receita usa `flow`, uma lista ordenada de itens
     `step`, `hitl` e `sink`. O parser continua aceitando o formato legado e o
     normaliza para `flow`, preservando ordem e compatibilidade.
  2. Receitas aceitam zero, um ou vários `triggers`. Zero gatilhos significa
     execução manual ou por rotina.
  3. `schedules` ganhou `trigger_type`, `trigger_config_json`, `target_kind` e
     `recipe_id`. Rotinas podem ser disparadas por CRON, pasta monitorada,
     Rsync, IMAP, Signal, webhook ou consulta SQL, e podem executar prompt ou
     receita.
  4. Gatilhos não-CRON reutilizam o barramento de I/O existente: a rotina cria
     uma `io_inbound_source` com `config.schedule_id`; o `ingest` reconhece o
     vínculo e executa a rotina sem duplicar watchers.
  5. O editor de receitas é local (sem loader remoto) para funcionar offline e
     nunca ficar preso em “Loading…”.
- **Consequências:** migrações `0020` e `0021`; `BACKLOG` é normalizado para
  `TODO`; receitas antigas continuam válidas; o pin de artefatos passa a vir da
  coluna `pinned`; o Attention Card consolida permissões, HITL, propostas,
  aprovações, bloqueios e falhas em uma única chamada.
