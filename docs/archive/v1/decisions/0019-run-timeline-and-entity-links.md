# ADR 0019 — Timeline de execução e referências navegáveis

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** o log de execução era o JSONL bruto em um `<pre>`, difícil de
  ler e sem duração/timestamps por etapa. Notificações e referências (runs,
  tarefas, artefatos, rotinas, agentes, arquivos) nem sempre levavam ao destino.

## Decisão

- O backend expõe `GET /api/runs/{id}/timeline`, que lê o JSONL e devolve uma
  estrutura estável:
  - `thinking`: raciocínio agregado, início/fim/duração e texto;
  - `executed`: ferramentas com id, nome, input, output, status, início/fim e
    duração;
  - `result`: texto final, status, timestamps e duração;
  - `steps`: passos com agente, modelo, tokens, custo, finish e duração;
  - `workspace_root` para transformar paths absolutos em links relativos.
- O parser tolera logs antigos/sem `created` e mantém `/api/runs/{id}/log` como
  fallback bruto.
- A UI usa um único `RunLogView`:
  - “Raciocínio” colapsado;
  - “Executado” colapsado, com ferramentas expansíveis;
  - “Resultado” sempre aberto;
  - modo bruto, copiar, atualizar e polling de 2,5s enquanto `RUNNING`.
- O mesmo componente aparece no drawer do Kanban e em `/runs/:id`.
- Uma página `/runs` lista execuções filtráveis e entra na sidebar (grupo
  Operação).
- `src/lib/links.ts` centraliza URLs de tarefa, run, artefato, rotina, agente,
  arquivo, settings e integrações. `EntityLink` renderiza qualquer referência
  clicável.
- `useFocusParam` implementa deep links com scroll + destaque (`?schedule=`,
  `?path=`, `?thread=`, `?section=`, `?kind=&id=`).
- O Toaster aceita `href` e `key`:
  - o corpo do toast vira link para o destino;
  - `key` deduplica rajadas (artefatos/reconexões);
  - runs, artefatos, Google, budget, engine e falhas de tarefa/rotina passam a
    apontar para a página correspondente.

## Consequências

- O log fica legível sem perder o bruto.
- Toda notificação acionável leva ao recurso real.
- Links compartilháveis funcionam por URL (run, tarefa, artefato, rotina,
  arquivo, agente, settings).
- O drawer e a página compartilham o mesmo renderizador e o mesmo parser
  testado, evitando divergência.
