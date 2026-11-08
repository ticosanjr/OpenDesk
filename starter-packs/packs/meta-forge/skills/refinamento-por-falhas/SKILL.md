---
name: refinamento por falhas
description: Melhora uma capacidade a partir de falhas observadas em transcrições, com mudanças mínimas e regression.
---
# Refinamento por falhas

## Quando usar
Quando uma capacidade falha em uso real ou em eval.

## Processo
1. Reproduza a falha em um caso isolado.
2. Classifique: descoberta, instrução ausente, instrução ambígua, ferramenta,
   contexto, ambiente ou grader injusto.
3. Corrija a causa com a menor mudança possível.
4. Rode o caso que falhou e a suíte de regression.
5. Leia a nova transcrição.
6. Se a mudança não melhorou nada, reverta.
7. Registre hipótese, evidência e resultado no changelog.

## Regras
- Não adicione regra sem falha que justifique.
- Não ajuste para um caso isolado quebrando outro.
- Prefira tornar a regra mais proeminente a duplicá-la.
- Uma mudança por vez.
