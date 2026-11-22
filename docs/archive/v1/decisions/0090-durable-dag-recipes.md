# 0090 — Receitas DAG com caminhos duráveis

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o driver de receitas era linear: um índice numérico por passo,
  sem grupos, e o resume recomeçava sem restaurar os outputs já concluídos.

## Decisão

1. Novos itens de fluxo:
   - `branch`: `when` + `then`/`else`;
   - `parallel`: `branches` + `join: all|any`;
   - `map`: expressão `items`, `item_var`, `max_concurrency` e `flow`.
2. Cada nó executável recebe um caminho estável:
   `decidir/t1`, `lote/b0/revisar`, `arquivos/0000-a/analisar`. O caminho é a
   chave de durabilidade (`recipe_run_nodes.node_path`), e `node_index` passa a
   ser apenas ordem de exibição.
3. Migração 0051 adiciona `node_path`, `parent_path`, `item_key` e
   `join_policy`; runs antigos usam o índice como caminho.
4. O executor DAG (`recipes/dag.rs`) caminha pela árvore e consulta o banco por
   caminho antes de executar. Nós `DONE`/`SKIPPED` não repetem, e grupos
   concluídos restauram os outputs dos filhos para o template seguinte.
5. `map` materializa um nó `map_item` por item e usa `buffer_unordered` com
   `max_concurrency` (1–8). Resultados são ordenados pela posição original e
   ficam em `steps.<map>.items[]` com `item` e `outputs`.
6. `parallel` executa branches com `join_all`; cada branch vê um snapshot dos
   outputs e devolve os seus, que são mesclados ao fim. `join: all` exige todos
   os branches; `join: any` aceita ao menos um.
7. Limites: profundidade 8, 500 nós, 16 branches, concorrência 8. HITL e sink
   dentro de `parallel`/`map` são rejeitados na validação — aprovação e entrega
   continuam no nível principal, fora de execução concorrente.
8. Templates aceitam `{{arquivo.path}}` além de `{{steps.arquivo.path}}` para
   deixar o `item_var` ergonômico.
9. Receitas sem grupos continuam no driver linear original; a rota é escolhida
   por `Recipe::has_dag()`.

## Consequências

- Resume de grupos concluídos não repete passos nem perde outputs.
- O mesmo fluxo suporta ramificação, fan-out fixo e fan-out dinâmico.
- Sub-receitas, versionamento e UI em árvore continuam pendentes em REQ-04/UI.
- HITL concorrente fica deliberadamente fora desta versão para não criar
  aprovações simultâneas ambíguas.
