# 0148 — G4: loop, switch e árvore de execução

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** o DAG tinha branch/parallel/map, mas não loops limitados nem
  múltiplos caminhos; a tela de execução mostrava nós em lista plana.

## Decisão

1. Parser ganha:
   - `loop` com `max_iterations` (1–100), `until` opcional e `flow`;
   - `switch` com `cases[].when`, `cases[].flow` e `default`.
2. `has_dag` roteia receitas com loop/switch para o executor de caminhos.
3. Cada iteração de loop ganha caminho durável próprio
   (`loop/0/passo`, `loop/1/passo`) e um nó marcador `iteration`. Isso permite
   retomar sem repetir iterações concluídas.
4. `switch` avalia os `when` em ordem e executa o primeiro verdadeiro; sem
   correspondência, executa `default`.
5. A UI de `RecipeRunDetailPage` monta a árvore por `parent_path`/`node_path`,
   com indentação e status por nó.
6. Testes cobrem parse, validação de limite, switch, loop de duas iterações e a
   existência dos caminhos duráveis.

## Consequências

- Receitas existentes continuam válidas; loop/switch são aditivos.
- `until` deve usar dados disponíveis nos outputs; após retomada, iterações já
  concluídas não reexecutam seus passos.
- HITL/sink dentro de loop/switch seguem a mesma restrição de grupos.
