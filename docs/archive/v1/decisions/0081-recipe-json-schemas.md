# 0081 — Contratos JSON Schema entre passos de receita

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** passos de receita trocavam texto livre; um passo posterior podia
  receber saída incompleta ou fora do formato sem falhar cedo.

## Decisão

1. **Schemas opcionais no passo.** `input_schema` e `output_schema` são JSON
   Schema validados no parse da receita; schema inválido rejeita o YAML.
2. **Entrada validada.** O contexto `{trigger, run, steps}` é validado contra
   `input_schema` antes do dispatch.
3. **Saída validada.** Quando há `output_schema`, o agente recebe a instrução de
   responder **somente JSON**. O engine extrai o último bloco cercado por
   ```json``` (ou o primeiro objeto) e valida.
4. **Retry de formatação.** Uma falha de schema tenta novamente no máximo uma
   vez, anexando o erro ao prompt; se persistir, o passo falha e o nó fica
   `FAILED` com a mensagem de validação.
5. **Output estruturado.** `steps.<id>.structured` guarda o JSON validado,
   além de `summary`, `artifacts` e `attempts`.

## Consequências

- Contratos entre passos deixam de ser convenção e passam a ser verificados.
- O passo seguinte pode confiar no formato sem parsing frágil.
- A UI permite editar os schemas no editor visual; o YAML continua sendo SSOT.
