# ADR 0021 — Contrato de layout e scroll

- **Status:** aceito
- **Data:** 2026-11-07
- **Contexto:** várias telas misturavam scroll da página com scroll interno de
  cards, `h-full` dentro de containers roláveis, `100vh` com headers e
  `overflow-hidden` sem `min-h-0`. Em alguns casos o conteúdo existia, mas o
  último item ficava inacessível. A versão do app também não aparecia no menu
  da bandeja.

## Decisão

- `html`, `body` e `#root` ocupam 100% da altura; o shell usa `100dvh`.
- `main` é o scroll principal, com `overscroll-behavior: contain`,
  `scrollbar-gutter: stable` e scrollbar visível.
- Páginas normais usam `PageBody` (fluxo + padding-bottom para o último item).
- Páginas split-pane usam `PageBodyFill`:
  - header/filtros `shrink-0`;
  - conteúdo `flex-1 min-h-0`;
  - cada painel tem seu próprio `overflow-y-auto`.
- Regras:
  - container flex que participa de altura precisa de `min-h-0`;
  - `overflow-hidden` só com filho direto rolável;
  - nunca `100vh`/`calc(100vh - Nrem)`;
  - tabelas e logs usam `overflow-auto` nos dois eixos;
  - drawers usam `h-dvh`;
  - modais usam `max-h-[calc(100dvh-2rem)]`.
- Kanban deixa as colunas crescerem e rola a página; o board rola apenas no
  eixo horizontal. Isso elimina o scroll aninhado que impedia chegar ao fim.
- Files, Artefatos, RunDetail, Studio e LogDrawer usam wrappers flexíveis com
  `min-h-0` + corpo rolável.
- A bandeja mostra `OpenDesk vX.Y.Z` no topo do menu e no tooltip.
- Uma auditoria Playwright percorre todas as rotas em 1100×700 e 1440×900,
  rola cada container até o fim e falha se houver conteúdo clipado por
  `overflow: hidden`.

## Consequências

- O último item de listas, tabelas, logs e formulários é sempre alcançável.
- Telas split-pane não dependem de cálculos frágeis de altura.
- A versão instalada fica visível sem abrir Configurações.
- Regressões de scroll passam a quebrar o gate local.
