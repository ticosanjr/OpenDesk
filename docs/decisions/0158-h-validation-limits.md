# 0158 — H1/H3/H4: limites de validação e release

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** o gate completo passou no piso acordado, mas H1/H3/H4 dependem de
  ambientes que este host Linux não possui.

## Decisão

1. **H1** foi tentado: o cliente Docker existe, mas o socket
   `/var/run/docker.sock` retorna _permission denied_ e Podman não está
   instalado. A imagem `packaging/sandbox` não foi construída nem publicada.
2. **H2** está concluído no piso acordado:
   - 54,38% total e 63,24% core;
   - gate com 53,8%/62,7%;
   - 620 testes Rust, 256 Vitest, 103 Playwright e goldens verdes.
3. **H3** permanece externo: Windows/macOS, Google dedicado, Telegram/Signal
   reais, WhatsApp/telefone, áudio e duas máquinas.
4. **H4** não será executado. Publicar um release final agora violaria o plano
   e produziria notas falsas de validação.

## Consequências

- O release final continua bloqueado por H1/H3 e pelos itens B2/B3, F1/F2/F4/F6.
- O estado atual é um checkpoint verificável, não um release.
- O próximo passo real exige acesso ao Docker (ou Podman) e às máquinas/contas
  de validação.
