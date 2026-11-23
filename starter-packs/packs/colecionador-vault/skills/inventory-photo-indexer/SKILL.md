---
name: Photo Indexer
description: Indexa fotos e itens.
---
# Photo Indexer

## Quando usar
Ao receber fotos de itens para o acervo.

## Processo
1. Renomeie cada foto com `categoria-id-sequencial-vista` e preserve o original.
2. Registre item, ângulo, iluminação, estado de conservação e data.
3. Detecte duplicatas por hash e fotos sem item legível.
4. Gere uma lista de pendências para fotos faltantes.

## Saída
CSV/JSON com caminho, item, ângulo, condição e hash; lista de pendências.

## Critérios de aceite
- [ ] Nenhum original é apagado.
- [ ] Toda foto tem item e ângulo.
- [ ] Duplicatas e fotos ilegíveis estão sinalizadas.
