# 0095 — `doctor --live` com probes reais

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o `--live` só verificava presença de configuração para Google,
  Telegram, Signal e WhatsApp. Um token inválido ou um CLI quebrado passava
  como "configured".

## Decisão

1. **Google**: quando a conta está conectada, `access_token` é chamado de
   verdade, exercitando o refresh token contra o OAuth do Google. Token
   rejeitado vira `error` com a mensagem do provedor.
2. **Telegram**: `getMe` é chamado com o token do bot; sucesso traz o
   `@username`, falha HTTP/rede vira `error`. Token presente com bot desabilitado
   vira `disabled`.
3. **Signal**: valida a conta configurada e executa `signal-cli --version`;
   CLI ausente/quebrado vira `error`.
4. **WhatsApp Cloud API**: consulta `graph.facebook.com/<version>/<phone_id>`
   com o token do secret e devolve o número/verified name. Provedor não
   Cloud API aparece como `configured` sem probe de rede; habilitado sem token
   vira `error`.
5. Probes continuam opt-in (`doctor --live`), com timeout curto (6s) e nunca
   logam tokens.
6. Testes cobrem a degradação para `not-configured`; o relatório JSON e o exit
   code do doctor permanecem iguais.

## Consequências

- O onboarding/diagnóstico distingue "configurado" de "funcionando".
- Nenhuma chamada de rede acontece no doctor padrão.
- Provedores futuros (Baileys) entram como `configured` até ganharem um probe
  próprio.
