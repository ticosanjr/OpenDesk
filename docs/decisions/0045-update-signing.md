# 0045 — Assinatura Ed25519 dos updates

- **Status:** aceito
- **Data:** 2026-11-11
- **Contexto:** o updater do OpenDesk baixa `latest.json` do GitHub e confia
  apenas no SHA-256 dos assets. Quem comprometer a release, o token ou o
  repositório pode trocar o binário e o hash juntos. O threat model já listava
  “update malicioso” sem assinatura criptográfica.

## Decisão

1. `scripts/release.sh` gera `latest.json` e assina os **bytes exatos** com
   Ed25519. A assinatura vira o asset `latest.json.sig`, com envelope:
   ```json
   {"algorithm":"ed25519","key_id":"<12 hex>","signature":"<base64>"}
   ```
2. A chave privada fica **fora do repositório**, por padrão em
   `~/.config/opendesk/update-signing.key` (0600), e pode ser apontada por
   `OPENDESK_UPDATE_SIGNING_KEY`.
3. A chave pública fica em `src-tauri/update-signing-public-key.txt` e é
   embutida no binário com `include_str!`.
4. `updates::check` e `download_and_stage` só aceitam releases que publiquem
   `latest.json` + `latest.json.sig` válidos. Releases antigas sem assinatura
   são recusadas com mensagem explícita, em vez de aceitar checksum fraco.
5. Ferramentas locais:
   - `opendesk keygen-update --out <chave> --public-out <pub>`
   - `opendesk sign-update <latest.json> <latest.json.sig> [--key <chave>]`
   - `opendesk verify-update <latest.json> <latest.json.sig>`
6. O `key_id` é o SHA-256 truncado da chave pública. Permite rotação futura sem
   ambiguidade; uma chave desconhecida falha fechado.

## Consequências

- Comprometer GitHub/token não basta para entregar um update: é preciso a chave
  privada offline.
- A chave privada precisa de backup seguro; perdê-la impede novos releases
  assinados (é possível rotacionar com uma nova chave pública e um release
  intermediário).
- O `release.sh` falha se a chave não existir ou se o binário de release não
  puder assinar/verificar.
- O primeiro release com assinatura deve ser publicado antes de os clientes
  antigos atualizarem; clientes novos recusam releases sem `latest.json.sig`.
- Testes cobrem assinatura válida, manifesto adulterado, key_id desconhecido e
  algoritmo inválido.
