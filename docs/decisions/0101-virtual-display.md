# 0101 — Display virtual isolado para computer-use

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** computer-use usava o `DISPLAY` real. Um agente podia mover o
  mouse ou digitar na sessão do usuário, mesmo com aprovação.

## Decisão

1. `computer_sessions` ganha `display` e `isolated` (migração 0055).
2. Sessões isoladas sobem um `Xvfb :N` próprio (N entre 90 e 129), com tela
   `1280x800x24` e `-nolisten tcp`; o processo fica num registry em memória e é
   encerrado em `end` ou no cleanup.
3. `POST /api/computer/sessions` aceita `isolated: true`. A sessão só é criada
   se Xvfb, `xdotool` e `import` estiverem disponíveis; caso contrário devolve
   conflito explícito.
4. `capabilities_for(display)` valida as ferramentas X11 e o backend vira
   `x11-isolated`. Captura e ações rodam com `DISPLAY=:N`, nunca com o display
   do usuário.
5. A captura X11 passa a usar `import -window root`, evitando o modo interativo
   do ImageMagick que podia esperar um clique.
6. A UI de Configurações → Sandbox/Computer-use tem a opção “display isolado
   (Xvfb)”, ligada por padrão; sem display real, ainda é possível criar a
   sessão se Xvfb estiver presente.
7. `end` idempotente encerra o Xvfb; o registry não deixa processos órfãos.

## Consequências

- Computer-use deixa de sequestrar o desktop do usuário no Linux.
- A allowlist de janela/app e o equivalente Windows/macOS continuam pendentes.
- Sessões sem isolamento explícito mantêm o comportamento anterior e dependem
  da aprovação humana.
- O teste de UI visual agora espera o carregamento assíncrono de Pacotes antes
  do golden, eliminando flutuação de screenshot.
