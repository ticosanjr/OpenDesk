---
name: Pantry DB Ops
description: Mantém inventário da dispensa.
---
# Pantry DB Ops

## Quando usar
Após compras ou antes de gerar a lista semanal.

## Processo
1. Leia o inventário atual e separe itens por local, quantidade e validade.
2. Subtraia o consumo informado e marque itens vencidos ou acabando.
3. Gere a lista de reposição sem duplicar o que já existe.
4. Preserve o histórico de alterações.

## Saída
Inventário atualizado, itens críticos e lista de compras sugerida.

## Critérios de aceite
- [ ] Nenhuma quantidade fica negativa.
- [ ] Itens vencidos são separados, não apagados.
- [ ] A lista de compras não duplica itens.
