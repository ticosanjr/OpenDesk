---
name: avaliação de capacidades
description: Projeta e lê avaliações de agentes com capability, regression, graders, pass@k/pass^k e revisão de transcrições.
---
# Avaliação de capacidades

## Quando usar
Antes de publicar uma skill/agente/comando e ao comparar uma mudança.

## Workflow
1. Colete 20–50 tarefas reais ou falhas observadas; comece com 3–5.
2. Escreva tarefa sem ambiguidade e uma solução de referência.
3. Misture casos positivos e negativos.
4. Escolha graders:
   - código/teste para resultado determinístico;
   - LLM com rubrica para texto;
   - humano para calibrar.
5. Defina pass@k para “acertar alguma vez” e pass^k para consistência.
6. Rode múltiplos trials em ambiente isolado.
7. Leia transcrições: falha real ou grader injusto?
8. Promova capability saturada para regression.

## Regras
- Não avalie só o caminho; avalie o resultado.
- Dê crédito parcial quando o resultado é multidimensional.
- Evite task ambígua: dois experts devem concordar no veredito.
- Nunca use só LLM judge sem calibrar.
