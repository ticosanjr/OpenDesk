# 0100 — Orçamento isolado por sub-receita

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** uma sub-receita podia consumir indefinidamente o orçamento do run
  pai. Não havia como limitar o custo de uma composição reutilizável.

## Decisão

1. `type: recipe` aceita `budget_usd` opcional (positivo e finito).
2. O `ScopeState` do executor DAG acompanha `spent_usd` e `budget_usd`.
3. Após cada step concluído, o custo estimado do `task_run` correspondente é
   somado ao escopo; se passar do limite, o run falha com `BadRequest` e a
   mensagem mostra gasto/limite.
4. A sub-receita recebe um escopo com o próprio limite; ao terminar, o gasto é
   somado ao pai e o limite do pai também é verificado. Orçamentos são,
   portanto, cumulativos e isolados ao mesmo tempo.
5. `parallel` e `map` herdam o limite no escopo local, cobram os filhos e somam
   o gasto no pai após o join. O total aparece em `spent_usd` nos outputs do
   grupo, permitindo auditoria e restauração no resume.
6. Map item concluído guarda `{outputs, spent_usd}` no nó durável; resume lê o
   valor para não perder o acumulado ao retomar.
7. Custo ausente/zero não bloqueia; a política usa o `estimated_cost_usd` já
   gravado pelos runs (real ou estimado por CLI, ADR 0086).

## Consequências

- Uma composição cara pode ser limitada sem afetar outras receitas.
- O limite é verificado após cada step, não antes do dispatch; um único step
  pode ultrapassar o teto e o run falha logo em seguida.
- Resume de um grupo não concluído pode subestimar o gasto de steps já DONE
  porque eles não são reexecutados; o teto continua sendo uma proteção, não uma
  cobrança contábil.
