# ADR 0027 — Ciclo de bandeja, abas, timeline ao vivo e controlador de fila

- **Status:** aceito
- **Data:** 2026-11-07

## Decisão

### Janela e bandeja
- A janela principal abre maximizada.
- O “X” esconde a janela por padrão (`close_to_tray = true`) e mantém daemon,
  fila, cron e notificações ativos.
- A configuração de fechamento continua disponível em Configurações →
  Notificações; desligada, o “X” encerra o app.
- Apenas a janela `main` participa do ciclo da bandeja. A janela Quick Capture
  fecha normalmente.
- “Sair” no menu da bandeja encerra de verdade.

### Abas
- Configurações: Geral, Engine, Atualizações, Notificações, Workspaces, Sobre.
- Integrações: SMTP, Google, Signal, Entregas, MCP e segurança.
- Kanban: Board, Lista, Fila.
- A aba ativa vive em `?section=`; apenas o painel ativo é montado.

### Timeline ao vivo
- O store escuta `RunStreamChunk` e o `RunLogView` recarrega com debounce.
- O backend publica chunks para passos, raciocínio, ferramentas, texto e status.
- O status começa em `RUNNING` em `session.execution.started`; `completed_at`
  só é preenchido em estado terminal.
- O payload do timeline inclui pasta de saída e artefatos do run, com links
  diretos para cada arquivo, para a pasta no sistema e para o explorador.

### Fila
- O controlador ganhou aba própria, contadores derivados da fila, lease,
  heartbeat, grupo de concorrência, prioridade, retry em lote e pasta de saída.
- O Kanban destaca a coluna de destino e faz auto-scroll durante o arraste.

## Consequências

- O app permanece em background sem depender de opção manual.
- Configurações e integrações ficam navegáveis sem rolagem longa.
- O usuário acompanha passos e ferramentas durante a execução.
- Entregáveis e pastas ficam a um clique do detalhe da execução.
- A fila é operável sem sair do Kanban.
