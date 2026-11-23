---
name: Policy Compliance Checker
description: Checa políticas de anúncio.
---
# Policy Compliance Checker

## Quando usar
Antes de publicar qualquer criativo ou copy.

## Processo
1. Liste as alegações explícitas e implícitas do anúncio.
2. Verifique antes/depois, atributos pessoais, saúde, finanças, urgência falsa e marcas de terceiros.
3. Para cada risco, classifique como `BLOQUEANTE`, `ATENÇÃO` ou `OK` e cite a frase.
4. Proponha uma reescrita segura quando houver bloqueio.

## Saída
Tabela `| Trecho | Risco | Severidade | Correção |`.

## Critérios de aceite
- [ ] Todo bloqueio tem correção sugerida.
- [ ] Nenhuma alegação fica sem fonte ou prova.
- [ ] O veredito final é `APTO` ou `NÃO APTO`.
