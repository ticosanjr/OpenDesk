---
name: Item Tagger Schema
description: Padroniza tags de acervo.
---
# Item Tagger Schema

## Quando usar
Ao cadastrar ou revisar itens do acervo.

## Processo
1. Use um vocabulário controlado por dimensão: tipo, época, material, tema, raridade e estado.
2. Aceite sinônimos apenas no campo de busca; a tag canônica fica em minúsculas.
3. Proíba tags vagas como `outros`, `legal` ou `antigo` sem qualificador.
4. Gere o JSON do item e valide contra o schema.

## Saída
JSON do item com tags canônicas e lista de termos rejeitados.

## Critérios de aceite
- [ ] Toda tag pertence a uma dimensão.
- [ ] O schema rejeita tags vazias ou duplicadas.
- [ ] O item tem pelo menos tipo, época e estado.
