---
name: criação de comandos
description: Cria comandos slash claros, determinísticos e testáveis com argumentos, pré-condições e validação.
---
# Criação de comandos

## Quando usar
Ao encapsular uma operação repetível que o usuário invoca com `/nome`.

## Workflow
1. **Interface:** nome curto, intenção única.
2. **Entrada:** `$ARGUMENTS` e formato esperado.
3. **Pré-condições:** o que precisa existir antes.
4. **Passos:** ordem determinística e ponto de validação.
5. **Saída:** arquivo, relatório ou resposta.
6. **Falhas:** o que fazer quando falta contexto ou ferramenta.
7. **Exemplos:** 2–3 usos reais.
8. **Teste:** rodar com argumento vazio, válido e inválido.

## Regras
- Um comando, uma intenção.
- Nunca execute ação destrutiva sem confirmação.
- Se depender de arquivo, diga como localizar.
- Se houver default, explique.

## Anti-padrões
- Comando que faz três coisas não relacionadas.
- Depender de contexto invisível.
- Ignorar argumento inválido.
- Publicar sem exemplo.
