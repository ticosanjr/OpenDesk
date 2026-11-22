# ADR 0036 — Origem de tarefas e deduplicação idempotente

## Contexto

Receitas, rotinas e o barramento de I/O criavam uma tarefa de Kanban por
execução. As tarefas não tinham origem, então o board e o Attention Card
mostravam dezenas de execuções automáticas misturadas com trabalho humano.
Também era possível criar/disparar a mesma tarefa várias vezes em sequência,
inclusive por duplo clique.

## Decisão

1. `tasks.origin` classifica `MANUAL`, `RECIPE`, `IO_BUS`, `CAPTURE` e
   `ROUTINE`. O Kanban abre em **Trabalho** (manual) e oferece uma visão
   **Automações**; o Attention Card só considera tarefas manuais.
2. Cada tarefa recebe `dedupe_key`, um SHA-256 de workspace, origem, título e
   descrição normalizados, modo, agente, modelo, esforço e tags ordenadas.
3. Tarefas ativas com a mesma chave são reutilizadas. Um índice único parcial
   garante isso mesmo sob concorrência.
4. Depois de concluída, a mesma chave só cria tarefa nova após a carência
   configurável (padrão **5 minutos**). `dedupe: false` (forks, swarms e
   “criar mesmo assim”) grava a chave como nula.
5. A fila também tem `dedupe_key` e índice único parcial para `QUEUED`/`RUNNING`.
   O enqueue usa identidade estável (tarefa/sessão ou rotina/horário), não o
   prompt renderizado.
6. Receitas e I/O bus respeitam `deduplicated`: não criam novo run nem
   redisparam a tarefa existente.
7. A origem é editável apenas pelo backend; a UI não permite forjar
   `origin` em criação manual.

## Consequências

- O board mostra trabalho humano por padrão; automações continuam visíveis na
  aba própria, nas receitas e em Execuções.
- Duplicatas ativas são impossíveis por construção, não só por verificação
  otimista.
- A carência pode suprimir uma repetição intencional de automação; o usuário
  controla o valor e pode desligá-la.
- Tarefas existentes foram backfilladas por tags (`recipe:`, `io-source:`,
  `quick-capture`).
