# 0064 — Web Push/VAPID para o PWA

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o PWA já recebia eventos por WebSocket, mas só enquanto a aba
  estava aberta. Faltava avisar aprovações, permissões, falhas e propostas sem
  depender de uma conexão viva.

## Decisão

1. **VAPID local**: o daemon gera um par P-256 na primeira execução e grava em
   `push_vapid.json` (0600). A chave privada nunca entra no SQLite nem no
   frontend; a pública é exposta em `GET /api/push/vapid`.
2. **Assinaturas em SQLite**: a migração 0044 cria `push_subscriptions` com
   endpoint único, `p256dh`, `auth`, rótulo e contadores de sucesso/falha. As
   chaves de criptografia não são devolvidas pela API de listagem.
3. **Registro opcional**: a UI de Configurações → Notificações mostra o painel
   apenas fora do Tauri e apenas quando o navegador suporta `PushManager`. O
   desktop continua usando notificações nativas.
4. **Fan-out por evento**: um dispatcher assina o `EventBus` e envia push para
   HITL, permissões, fim de execução, propostas e alertas de orçamento. O
   service worker agrupa por `tag` e abre a rota relevante ao clicar.
5. **Expiração automática**: endpoints removidos pelo serviço de push
   (404/410) são apagados; outras falhas incrementam `failure_count` sem
   bloquear os demais dispositivos.
6. **Teste sob demanda**: `POST /api/push/test` envia um aviso de teste e
   devolve quantos dispositivos aceitaram.

## Consequências

- O celular recebe aprovações e falhas mesmo com a aba fechada, desde que o
  PWA tenha sido instalado/assinado ao menos uma vez.
- Push depende de HTTPS (ou localhost). Em LAN/Tailscale, publique via reverse
  proxy TLS; o daemon continua em `127.0.0.1`.
- Sem multi-tenant: todas as assinaturas pertencem ao único operador do
  workspace.
