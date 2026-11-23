---
name: Display URL Generator
description: Cria display URL coerente.
---
# Display URL Generator

## Quando usar
Ao montar anúncios que exibem uma URL curta e legível.

## Processo
1. Extraia domínio e caminho real do destino; nunca invente um domínio.
2. Use no máximo dois segmentos em minúsculas, com hífen e sem parâmetros.
3. Garanta que o texto exibido não prometa algo ausente na landing page.
4. Compare a URL exibida com a final e registre a diferença.

## Saída
`display_url`, `final_url` e uma justificativa de uma linha.

## Critérios de aceite
- [ ] A display URL pertence ao domínio do anúncio.
- [ ] Não há promessa de preço, prazo ou resultado não comprovado.
- [ ] O caminho é estável e não contém UTM.
