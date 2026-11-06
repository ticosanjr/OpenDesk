# ADR 0005 — Secret store local criptografado

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** SMTP e, futuramente, Google Workspace precisam de senhas e
  refresh tokens. O OpenDesk roda em desktop Linux, Windows e macOS; nem todo
  Linux tem Secret Service/DBus disponível, e o app não pode depender de um
  serviço externo para iniciar.

## Decisão

Segredos são gravados em `~/.local/share/opendesk/secrets/` com AES-256-GCM,
nonce aleatório, base64 e permissão `0600`. A chave é derivada por SHA-256 de
um salt fixo do app + machine-id (`/etc/machine-id` ou
`/var/lib/dbus/machine-id`), com fallback para hostname.

A interface `SecretStore` (`set/get/delete`) é isolada para permitir um backend
keyring/Secret Service no futuro sem mudar os chamadores.

## Consequências

- Segredos não aparecem em texto puro no SQLite, nos logs ou no Git.
- Copiar o banco para outra máquina não permite decifrar os segredos.
- Trocar o machine-id invalida os segredos; o usuário precisa recadastrar
  SMTP/Google.
- O modelo de ameaça é “proteger contra leitura casual do arquivo/banco”, não
  contra um atacante com execução na mesma conta de usuário.
- Keyring nativo continua sendo o alvo; este store é o fallback confiável.
