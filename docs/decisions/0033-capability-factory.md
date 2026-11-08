# 0033 — Fábrica de Capacidades como workspace default

- **Status:** aceito
- **Data:** 2026-11-08
- **Contexto:** criar skills, agentes e comandos exigia conhecimento de formato,
  avaliação e progressive disclosure. O app já tinha Studio e pacotes, mas não
  um ponto de partida metodológico.
- **Decisão:**
  1. O pack `meta-forge` codifica a metodologia eval-driven: baseline, evals de
     capability e regression, solução de referência, draft mínimo, red team e
     iteração por falhas.
  2. Na primeira execução, o OpenDesk cria uma única workspace
     `Fábrica de Capacidades` em `workspaces_root`, instala `essenciais` e
     `meta-forge` e grava `factory_workspace_created`. Workspaces existentes
     não são substituídas nem perdem o foco.
  3. O template `fabrica-capacidades` permite recriar o workspace manualmente.
  4. O workspace inclui agentes, skills, comandos, receitas, rotina de auditoria
     inativa e pastas `capabilities/*`.
  5. O tray usa o mesmo mark 32×32 de `public/opendesk.svg`; os ícones de
     bundle/janela são gerados a partir do mesmo SVG.
- **Consequências:** novo pack e template; bootstrap idempotente; teste de
  criação única; `OPENDESK_WORKSPACES_ROOT` permite isolar testes.
