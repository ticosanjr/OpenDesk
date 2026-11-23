---
name: Bulk Purchase Evaluator
description: Compara custo por unidade.
---
# Bulk Purchase Evaluator

## Quando usar
Antes de comprar em quantidade maior.

## Processo
1. Normalize preço e quantidade para a mesma unidade (kg, L ou unidade).
2. Calcule custo por unidade e desconto versus a embalagem menor.
3. Considere validade, espaço, consumo mensal e risco de desperdício.
4. Calcule o ponto de equilíbrio em semanas.

## Saída
Tabela com preço unitário, desconto, validade e recomendação.

## Critérios de aceite
- [ ] As unidades foram normalizadas.
- [ ] O desperdício estimado entra na conta.
- [ ] A recomendação diz comprar ou não comprar.
