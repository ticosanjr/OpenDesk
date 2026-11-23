---
name: Aisle Categorizer
description: Classifica por corredor.
---
# Aisle Categorizer

## Quando usar
Ao montar a lista de compras para uma loja específica.

## Processo
1. Receba a ordem real dos corredores da loja.
2. Classifique cada item por setor e corredor, mantendo perecíveis por último.
3. Una itens repetidos e converta unidades para a embalagem vendida.
4. Sinalize itens sem corredor conhecido para revisão humana.

## Saída
Lista ordenada por corredor, com quantidade e unidade.

## Critérios de aceite
- [ ] A ordem segue o layout informado.
- [ ] Perecíveis e congelados ficam no fim.
- [ ] Itens sem corredor estão marcados.
