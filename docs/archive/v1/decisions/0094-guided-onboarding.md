# 0094 — Onboarding guiado com validação real

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** a configuração inicial exigia alternar entre Configurações,
  Integrações, Studio e Pacotes, sem uma visão única do que faltava. O usuário
  precisava descobrir sozinho se o motor, MCP, pacote base e canais estavam ok.

## Decisão

1. `GET /api/onboarding` monta um checklist a partir do estado real:
   workspace ativo, motor (daemon ou binário), MCP `opendesk-control` no
   `opencode.jsonc`, skill do pacote base, Google, Telegram, Signal, WhatsApp,
   passkeys e atalho do desktop.
2. Cada item traz `status` (`done`/`todo`), `detail`, `href` e `required`.
   Essenciais: workspace, motor, MCP e pacote base. Integrações e desktop são
   opcionais e não bloqueiam `complete`.
3. A UI `Primeiros passos` (Topbar) abre um diálogo com Essenciais/Opcionais,
   progresso e botão "Verificar". Clicar em um item navega para a tela que o
   resolve.
4. Não há mock: todos os estados vêm do banco, do sistema de arquivos, do
   daemon e dos segredos. Um item só fica `done` quando a condição real é
   satisfeita.
5. O diálogo é verificado no Playwright com um workspace real e o modal
   reflete o caminho do workspace.

## Consequências

- O usuário fecha a configuração inicial sem abrir terminal.
- O checklist é idempotente e pode ser reaberto a qualquer momento.
- A entrada na Topbar é um elemento persistente; dois goldens (Kanban e Pacotes
  em 1440) foram regravados por causa do novo botão.
- Passkey e atalho de desktop aparecem como opcionais para não bloquear quem
  usa só o desktop local.
