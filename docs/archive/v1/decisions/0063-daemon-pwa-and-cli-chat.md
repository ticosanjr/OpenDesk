# 0063 — Daemon configurável, PWA e CLI de chat (onda 1, parte 1)

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o `opendeskd` já existia como binário headless, mas o host era
  fixo em localhost, a UI não era servida pelo daemon e não havia caminho de
  acesso remoto nem chat no terminal.

## Decisão

1. **Bind configurável**: `OPENDESK_API_HOST` (padrão `127.0.0.1`) e
   `OPENDESK_API_PORT`. O token continua obrigatório e fica em `api.json` 0600.
2. **PWA servida pelo Axum**: quando `OPENDESK_WEB_DIR` aponta para o build Vite,
   o daemon serve a SPA com fallback para `index.html`, além de
   `manifest.webmanifest` e `sw.js`.
3. **Login remoto por sessão**: em navegador (fora do Tauri), a UI pede o token
   da API e o troca por um token de sessão (`/api/auth/session`). O token de
   sessão tem TTL, é armazenado como hash e pode ser revogado; o token local
   continua válido para o desktop.
4. **Cliente API same-origin**: `api.ts` passa a usar `baseUrl` quando servido
   pelo daemon; WebSocket usa `ws/wss` do mesmo host.
5. **CLI de chat e operação**: `opendesk chat [mensagem]` cria/reutiliza uma
   conversa, envia a mensagem e imprime a resposta; sem mensagem, abre um loop
   de linhas. `opendesk approvals list/resolve` trata HITL de tarefas e
   `opendesk diff <run>` imprime os artefatos e diffs no terminal.
6. **Empacotamento**: serviços de exemplo systemd/launchd e instruções Windows,
   com bind local e acesso remoto via Tailscale/WireGuard/reverse proxy.

## Pendências da onda 1

- Extração física de `opendesk-core` e cliente Tauri 100% remoto.
- Passkey/WebAuthn (sessões persistentes por token já implementadas).
- Web Push/VAPID.
- TUI completa e inspeção de diffs no terminal.

## Consequências

- O daemon já pode servir uma PWA funcional em rede local/Tailscale com token.
- O desktop não muda de comportamento.
- A autenticação ainda é single-user por token; passkey entra na próxima parte.
