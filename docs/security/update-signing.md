# Assinatura de updates

O OpenDesk assina o manifesto `latest.json` com Ed25519. O app embute apenas a
chave pública e recusa releases sem `latest.json.sig` válido.

## Chaves

- Privada: `~/.config/opendesk/update-signing.key` (0600). Nunca versionar.
- Pública: `src-tauri/crates/opendesk-enterprise/src/update-signing-public-key.txt` (versionada).
- Alternativa de caminho: `OPENDESK_UPDATE_SIGNING_KEY=/caminho/chave`.

Gerar um par novo (substitui a chave pública do repositório):

```bash
opendesk keygen-update \
  --out ~/.config/opendesk/update-signing.key \
  --public-out src-tauri/crates/opendesk-enterprise/src/update-signing-public-key.txt
```

Guarde a chave privada offline e faça backup. Sem ela não é possível assinar
novos releases.

## Release

`scripts/release.sh` faz automaticamente:

1. gera `latest.json`;
2. executa `opendesk sign-update latest.json latest.json.sig`;
3. executa `opendesk verify-update` para conferir;
4. publica os dois assets no GitHub.

Para assinar manualmente:

```bash
opendesk sign-update releases/0.43.0/latest.json releases/0.43.0/latest.json.sig
opendesk verify-update releases/0.43.0/latest.json releases/0.43.0/latest.json.sig
```

## Rotação

O envelope inclui `key_id` (SHA-256 truncado da chave pública). Para rotacionar:

1. gere um novo par;
2. publique um release intermediário assinado com a chave antiga **ou** oriente
   a atualização manual;
3. troque a chave pública no repositório e recompile;
4. publique o release seguinte assinado com a chave nova.

Clientes com chave antiga recusam a chave nova até atualizar; por isso a
rotação exige uma janela de transição.

## Limitações

- A assinatura cobre o manifesto (versão, assets e SHA-256). O download do
  asset continua verificando o SHA-256 declarado.
- Releases anteriores à assinatura são recusados pelo cliente novo.
- A chave privada não é gerenciada pelo app; é uma responsabilidade do operador
  de release.
