---
name: UTM Tagger
description: Padroniza links e origem.
---
# UTM Tagger

## Quando usar
Antes de publicar links em anúncios, e-mail, redes ou parceiros.

## Processo
1. Defina o padrão: `utm_source`, `utm_medium`, `utm_campaign`, `utm_content` e `utm_term` quando fizer sentido.
2. Use minúsculas, hífen e nomes estáveis por campanha.
3. Preserve parâmetros existentes e não duplique UTMs.
4. Valide o link final e registre a tabela de origem.

## Saída
Link marcado, tabela de parâmetros e alerta de conflito.

## Critérios de aceite
- [ ] Todos os links da campanha seguem o mesmo padrão.
- [ ] Nenhum parâmetro pessoal ou sensível entra na URL.
- [ ] O link final responde 200 antes de ser publicado.
