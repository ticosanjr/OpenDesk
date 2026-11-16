# Threat model — API local, agentes e navegador

## Ativos

- Banco SQLite operacional e backups.
- Token da API local (`api.json`, 0600).
- Prompts, respostas, artefatos e logs de execução.
- Credenciais de integrações (SMTP, Google, Signal) no armazenamento local.
- Navegador real do usuário exposto pelo OpenCode Browse.
- Configuração nativa do OpenCode (`.opencode/`, `opencode.jsonc`).
- Tokens de sessão remota e tokens de nós remotos (somente hash em SQLite).
- Áudio de voz (temporário) e capturas de computer-use.

## Fronteiras de confiança

1. **Processos locais do mesmo usuário** podem ler `api.json` e chamar a API.
2. **OpenCode daemon** executa ferramentas e sessões fora do processo do
   OpenDesk.
3. **Servidores MCP** rodam com privilégios do usuário.
4. **Extensão Chromium/Browse** controla o navegador logado do usuário.
5. **Provedores externos** recebem prompts e conteúdo do workspace.
6. **Conteúdo web** lido por agentes é não confiável e pode conter prompt
   injection.

## Ameaças e mitigações atuais

| Ameaça | Mitigação atual | Lacuna |
|---|---|---|
| Leitura do token por outro processo | arquivo 0600, CORS restrito, Bearer | sem peer auth por socket Unix |
| CSRF/DNS rebinding contra a API local | CORS restrito, Host/Origin local | sem rate limit |
| Path traversal ao abrir/restaurar arquivos | validação de caminhos, zip-slip, symlinks | auditar todos os endpoints |
| Backup corrompido | SHA-256 + validação de ZIP | sem assinatura do backup |
| Prompt injection em página web | skill Browse orienta allowlist/handoff; agente não contorna captcha | allowlist não é obrigatória por padrão |
| MCP malicioso | inventário explícito, catálogo sem segredos, toggle/remoção, health | sem quarentena automática |
| Nó remoto comprometido | token próprio com hash, escopo restrito ao protocolo, lease e posse do job | sem mTLS entre central e nó |
| Sandbox insuficiente | modos explícitos, allowlist, env scrubbing e bwrap sem fallback | `strict` depende de bwrap instalado |
| Computer-use indevido | sessão com aprovação opcional, ações contadas, texto não persistido | sem bloqueio por app/janela |
| Voz vazando áudio | temporário apagado após STT, limite de 25 MB | motor externo pode gravar logs próprios |
| Vazamento de prompts/segredos em logs | evidência truncada, sem prompts completos no aprendizado | logs de run ainda podem conter conteúdo |
| Custo descontrolado | budget por workspace/agente, quota, HITL | alerta proativo em 80/100% |
| Agente age no navegador real | abas isoladas, handoff para login/2FA, sem armazenar cookies | auditoria de sessões no OpenDesk |
| PWA servida pelo daemon | login por sessão revogável, CORS, cabeçalhos de segurança e CSP | reverse proxy TLS é responsabilidade do operador |
| Roubo de token do PWA | sessão revogável + passkey WebAuthn (sem token permanente) | comprometer o navegador ainda concede a sessão ativa |
| Replay de cerimônia WebAuthn | estado no servidor, uso único, TTL de 10 min | depende de origem HTTPS correta |
| Webhook WhatsApp forjado | assinatura HMAC-SHA256 com app secret + verify token | app secret precisa ser configurado |
| Artefato remoto fora do workspace | worker envia caminho relativo; central rejeita absoluto e `..` | symlink dentro do workspace ainda é seguido |
| Token de nó vazado | hash no banco + rotação imediata pela UI/API | sem mTLS entre central e worker |
| Update malicioso | assinatura Ed25519 do `latest.json` + SHA-256 do asset; chave privada offline | rotação de chave exige release intermediário |

## Recomendações prioritárias

1. ~~Rate limit e limite de corpo por rota na API local~~ — implementados.
2. ~~Auditoria de chamadas sensíveis~~ — implementada; ampliar filtros por cliente/MCP.
3. Allowlist de domínios obrigatória para tarefas `needs-browser` de alto risco.
4. Sandbox/quarentena para MCPs e packs antes de habilitar.
5. Criptografia em repouso para workspaces sensíveis e retenção configurável.
6. ~~Assinatura de updates~~ — implementada em 0.43.0 (ADR 0045).
7. Testes de abuso: zip bomb, symlink, token vazado, prompt injection,
   ferramenta MCP destrutiva.
