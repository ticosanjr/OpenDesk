# ADR 0010 — Atualizações do OpenCode e do OpenDesk

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** o usuário precisa atualizar o motor e o próprio app pela UI,
  sem CI e com releases privadas. O GitHub `/releases/latest` do OpenCode ainda
  aponta para V1 (`v1.19.5`) mesmo com V2 instalado (`v2.2.5`).

## Decisão

### OpenCode

- Listar `releases?per_page=50` e escolher o maior semver estável com
  `major == 2`; ignorar `process-daemon-*` e pré-releases.
- Executar `opencode upgrade --method <método> <versão>` usando o método
  detectado pelo caminho do binário (`curl`, `npm`, `bun`, `brew`).
- Pausar a fila, recusar quando houver job `RUNNING`, reiniciar o daemon e
  validar a versão instalada.
- Em caso de falha, manter o daemon anterior e devolver o output.

### OpenDesk

- `scripts/release.sh` publica `latest.json` com versão, notas, data, tamanho e
  SHA256 de cada asset, além de criar a GitHub Release privada via `gh`.
- O app lê a release com token fine-grained (`Contents: Read`,
  `Metadata: Read`) guardado no secret store ou `gh auth token`.
- O asset é baixado pela API do GitHub, verificado por SHA256 e só então
  instalado.
- **AppImage / binário portátil:** helper `setsid` espera o PID atual sair,
  copia o arquivo verificado, mantém `.bak` e relança o app.
- **`.deb`:** baixa para `~/Downloads` e mostra `sudo dpkg -i`; o app nunca
  eleva privilégio sozinho.
- **Build de desenvolvimento:** apenas informa que a auto-substituição é
  desabilitada.

## Consequências

- Não há CI; o gate é local (`release.sh` + testes).
- A release privada exige token; sem token, o app continua funcionando e mostra
  instrução clara.
- O rollback manual usa o `.bak`; o rollback automático de versão fica para uma
  próxima iteração.
- macOS/Windows usam o mesmo contrato de `latest.json` e assets, com instalação
  específica por plataforma em iterações futuras.
