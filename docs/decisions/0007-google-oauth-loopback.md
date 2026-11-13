# ADR 0007 — OAuth Google loopback e credenciais

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** o OpenDesk é um app desktop local. OAuth para Google Workspace
  precisa de um redirect URI estável, não pode depender de um servidor público e
  deve guardar refresh tokens com segurança.

## Decisão

- Fluxo Authorization Code + PKCE com redirect loopback fixo em
  `http://127.0.0.1:53682/callback` (override por `OPENDESK_OAUTH_PORT`).
- O callback é servido por um pequeno listener Axum iniciado sob demanda; o
  `state` aleatório mapeia para workspace, verifier e redirect.
- `client_id` fica em `app_settings`; `client_secret` e refresh tokens ficam no
  `SecretStore` local (ADR 0005), por workspace.
- Access tokens ficam em cache em memória com expiração; refresh é feito no
  primeiro uso após expirar.
- Escopos: `openid email profile`, `drive.file`, `documents`,
  `calendar.events`, `calendar.readonly`, `gmail.send`, `gmail.readonly`,
  `tasks`. Contas conectadas antes do `gmail.readonly` precisam reconectar para
  liberar a busca de threads.

## Consequências

- O usuário precisa criar um OAuth client do tipo Desktop no Google Cloud e
  cadastrar o callback mostrado na UI.
- Nenhum servidor público é necessário.
- O refresh token é revogável pelo usuário; a UI oferece “Revogar”.
- O callback fica restrito a 127.0.0.1 e valida `state`; não há troca de código
  sem um flow pendente.
- Google Keep não usa este fluxo; a API é enterprise e fica fora do escopo.
