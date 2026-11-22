# 0083 — Passos condicionais em receitas (`when`)

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o primeiro passo rumo a DAGs era permitir que um passo fosse
  pulado com base em saída anterior, gatilho ou decisão, sem criar um novo
  dialeto de fluxo.

## Decisão

1. **Campo `when` opcional no passo.** Vazio/`true` executa; `false` pula.
2. **Expressões simples e seguras.** Suporta:
   - caminho (`trigger.approved`, `steps.plan.structured.status`) com
     verificação de veracidade;
   - igualdade/desigualdade (`==`, `!=`) com string entre aspas ou não;
   - negação `not <caminho>`.
   Não há `eval`, funções arbitrárias ou acesso fora de `trigger`,
   `run` e `steps`.
3. **Nó `SKIPPED`.** O passo pulado não executa o agente; o nó durável fica
   `SKIPPED` com motivo e o resume o ignora.
4. **UI.** O editor visual mostra “Executar quando (opcional)” no passo.
5. **Escopo.** `parallel` e `map/fan-out` continuam pendentes; esta decisão
   cobre o desvio condicional linear sobre os nós duráveis existentes.

## Consequências

- Receitas já conseguem ramificar sem duplicar fluxo inteiro.
- A expressão é deliberadamente pequena; contratos JSON (`REQ-03`) cobrem o
  formato dos dados usados na condição.
