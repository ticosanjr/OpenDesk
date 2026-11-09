# Threat model — API local, agentes e navegador

## Ativos

- Banco SQLite operacional e backups.
- Token da API local (`api.json`, 0600).
- Prompts, respostas, artefatos e logs de execução.
- Credenciais de integrações (SMTP, Google, Signal) no armazenamento local.
- Navegador real do usuário exposto pelo OpenCode Browse.
- Configuração nativa do OpenCode (`.opencode/`, `opencode.jsonc`).

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
| MCP malicioso | instalação explícita, escopo por workspace, permissões | sem sandbox/quarentena |
| Vazamento de prompts/segredos em logs | evidência truncada, sem prompts completos no aprendizado | logs de run ainda podem conter conteúdo |
| Custo descontrolado | budget por workspace/agente, quota, HITL | alerta proativo em 80/100% |
| Agente age no navegador real | abas isoladas, handoff para login/2FA, sem armazenar cookies | auditoria de sessões no OpenDesk |
| Update malicioso | checksum do release | sem assinatura criptográfica |

## Recomendações prioritárias

1. Rate limit e limite de corpo por rota na API local.
2. Auditoria de chamadas sensíveis: restore, MCP, browser, credenciais.
3. Allowlist de domínios obrigatória para tarefas `needs-browser` de alto risco.
4. Sandbox/quarentena para MCPs e packs antes de habilitar.
5. Criptografia em repouso para workspaces sensíveis e retenção configurável.
6. Assinatura de updates e verificação no cliente.
7. Testes de abuso: zip bomb, symlink, token vazado, prompt injection,
   ferramenta MCP destrutiva.
