# 0073 — Passkeys (WebAuthn) para o acesso remoto

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** a sessão remota por token funcionava, mas exigia colar o token
  de `api.json` em cada dispositivo. O programa v1.0 previa passkey/WebAuthn
  como evolução natural e ela ficou pendente no corte 1.0.0.

## Decisão

1. **WebAuthn de verdade, single-user.** Uma credencial por dispositivo, todas
   pertencentes ao operador local (`user unique id` estável). Não há cadastro
   multiusuário.
2. **Origem derivada da requisição.** O RP ID vem do header `Origin`/`Host`
   (ou de `OPENDESK_WEBAUTHN_ORIGIN`/`RP_ID`). HTTP só é aceito em `localhost`;
   IP é recusado porque WebAuthn não aceita RP ID numérico. A UI explica quando
   falta HTTPS ou domínio.
3. **Cerimônias no servidor.** `passkey_ceremonies` guarda o estado de
   `PasskeyRegistration`/`PasskeyAuthentication` com TTL de 10 minutos,
   uso único e limpeza na manutenção. Nunca vai para cookie/localStorage.
4. **Credenciais no SQLite.** `passkey_credentials` guarda o JSON do `Passkey`,
   ID da credencial (base64url), rótulo e último uso. O counter é atualizado
   quando `needs_update` é verdadeiro.
5. **Login emite sessão comum.** `finish_login` valida presença/verificação do
   usuário e cria uma `auth_session` revogável de 30 dias — passkey não vira um
   token permanente. Logout e revogação continuam funcionando.
6. **Fallback por token.** Token da API continua disponível para primeiro
   acesso, CLI, emergência e ambientes sem HTTPS.
7. **UI.** O gate remoto mostra “Entrar com passkey” quando há credencial e a
   origem é segura; Configurações → Segurança permite cadastrar, listar e
   remover passkeys.

## Consequências

- O PWA deixa de depender de copiar segredo para cada login, mantendo o token
  como recuperação.
- Passkeys em LAN/Tailscale sem domínio exigem um reverse proxy TLS com nome
  válido; IP não é suportado pelo padrão.
- Comprometer o navegador ainda compromete a sessão emitida; a revogação e o
  TTL continuam sendo a defesa.
