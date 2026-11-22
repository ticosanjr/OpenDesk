# 0097 — Sandbox em container com imagem única

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** `strict` dependia de bubblewrap e do PATH do host. Não havia
  isolamento portátil para Windows/macOS nem uma imagem reprodutível.

## Decisão

1. `sandbox.mode=container` usa Docker ou Podman (Podman tem prioridade no
   PATH) e uma imagem configurável (`sandbox.image`, padrão
   `opendesk/sandbox:latest`).
2. O container roda com:
   - `--rm --init`, `--cap-drop ALL`, `no-new-privileges`;
   - root filesystem read-only e `/tmp` tmpfs;
   - workspace montado read-write em `/workspace`, cwd `/workspace`;
   - `--network none` quando `network=false`, senão `bridge`;
   - `--pids-limit 512`;
   - UID/GID do usuário local em Unix (evita arquivos root no workspace).
3. O comando absoluto do harness é reduzido ao basename e resolvido pelo PATH
   da imagem. Harnesses devem ser instalados na imagem.
4. `OPENDESK_HARNESS/AGENT/WORKSPACE/PARENT_PID` são propagados com `-e VAR`
   (Docker/Podman leem do ambiente do processo pai).
5. `GET /api/sandbox` passa a informar `container_runtime`; a UI mostra `bwrap`
   e o runtime de container e permite escolher o modo/imagem.
6. Sem runtime de container o run falha com `Conflict` explícito; nunca cai
   silenciosamente para execução sem sandbox.
7. `packaging/sandbox/Dockerfile` define a base com OpenCode, Node, Python,
   Git, jq, ripgrep e bubblewrap.

## Consequências

- O mesmo contrato de sandbox vale em Linux, macOS e Windows com Docker Desktop.
- A imagem é responsável por versões de harness; o host não vaza PATH/binários.
- Validação end-to-end em macOS/Windows e a publicação da imagem no release
  continuam pendentes.
