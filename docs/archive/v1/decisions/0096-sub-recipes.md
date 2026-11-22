# 0096 — Sub-receitas com isolamento e detecção de ciclo

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** uma receita não podia reutilizar outra. Fluxos comuns (triagem,
  coleta, revisão) eram copiados entre arquivos, perdendo correções e duplicando
  manutenção.

## Decisão

1. Novo item de fluxo `type: recipe` com `recipe` (id no mesmo workspace),
   `with` opcional e `when` opcional.
2. `with` é um objeto de templates; cada string é renderizada com
   `trigger`/`run`/`steps` do chamador e vira o `trigger` da sub-receita.
3. A sub-receita roda no mesmo run/task, mas com `ScopeState` isolado:
   - ela vê um snapshot dos outputs do pai;
   - os passos dela não sobrescrevem ids do pai;
   - o resultado fica em `steps.<call_id> = { recipe, outputs }`.
4. Os nós duráveis usam o caminho `call_id/...`, então resume reaproveita passos
   concluídos dentro da sub-receita.
5. A pilha de receitas fica no `ScopeState`: ciclo (`a -> b -> a`) e profundidade
   acima de 8 são rejeitados antes de executar qualquer passo.
6. Sub-receita não é permitida dentro de `parallel`/`map`, pois HITL/sink
   continuam proibidos em grupos concorrentes; a chamada fica no nível principal.
7. A sub-receita precisa estar indexada no workspace e ser parseável; receitas
   desabilitadas também podem ser chamadas explicitamente (o gate de
   habilitadas vale para triggers).

## Consequências

- Um fluxo pode ser composto e corrigido em um único lugar.
- O resume preserva o trabalho já feito dentro da sub-receita.
- Orçamento isolado por sub-receita e versionamento/diff/rollback no Studio
  continuam pendentes em REQ-04.
- O nome da receita nos pedidos HITL dentro de uma sub-receita usa o nome da
  sub-receita, porque o contexto é criado a partir dela.
