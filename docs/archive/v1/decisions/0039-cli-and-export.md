# ADR 0039 — CLI headless e export portátil de sessão

## Contexto

O OpenDesk tinha uma API local e um modo `--headless`, mas nenhuma superfície
de linha de comando e nenhuma forma de exportar uma execução para fora do app.
Isso limitava automação, CI, revisão offline e comparação com ferramentas como
Pi.

## Decisão

- O binário `opendesk` ganha subcomandos que falam com a API local já existente:
  `status`, `workspaces list`, `tasks list/create/run`, `runs list/show`,
  `queue list` e `export`.
- O CLI **não** abre o banco diretamente nem duplica execução. Ele lê
  `api.json` (0600) dos diretórios do app e usa Bearer token.
- Export de sessão é derivado do timeline estruturado do run, não do log bruto:
  Markdown, HTML autocontido e JSON completo.
- O mesmo endpoint `/api/runs/{id}/export` alimenta o botão da UI e o CLI.
- HTML usa escape de conteúdo e CSS embutido; JSON preserva o contrato do
  timeline.
- O formato padrão é Markdown; formatos desconhecidos retornam 400.

## Consequências

- OpenDesk pode ser operado sem GUI em scripts e CI.
- Sessões podem ser arquivadas e revisadas fora do app.
- O CLI depende do app/headless estar rodando; não inicia um segundo processo
  nem disputa SQLite.
- O export é read-only e não altera runs, artefatos ou configuração.
