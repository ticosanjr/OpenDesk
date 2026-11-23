---
name: Ad Script Formatter
description: Formata roteiro por cena.
---
# Ad Script Formatter

## Quando usar
Quando o roteiro já existe e precisa virar uma tabela de cenas executável.

## Entradas obrigatórias
- Roteiro aprovado, duração alvo, oferta e CTA.
- Formato de destino (Reels, Shorts, feed ou TV).

## Processo
1. Divida o roteiro em cenas de 3 a 8 segundos, sem cortar a lógica da oferta.
2. Para cada cena, registre tempo, enquadramento, fala, texto na tela e som.
3. Marque o gancho nos três primeiros segundos e o CTA no último bloco.
4. Sinalize cenas que dependem de gravação, locução ou material de apoio.

## Saída
Tabela Markdown: `| Cena | Tempo | Visual | Fala | Texto na tela | CTA |`.

## Critérios de aceite
- [ ] A soma dos tempos fecha com a duração alvo.
- [ ] O gancho aparece antes de 3 s e o CTA é único.
- [ ] Nenhuma cena depende de alegação sem prova.
