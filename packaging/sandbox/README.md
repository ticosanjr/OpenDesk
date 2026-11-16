# Imagem única do sandbox

```bash
docker build -t opendesk/sandbox:latest packaging/sandbox
```

Depois escolha `sandbox.mode = container` em Configurações → Sandbox. A imagem
padrão já traz `opencode-ai`, Node, Python, Git, ripgrep e bubblewrap; ajuste o
campo `image` (política global ou por workspace) para outra tag.

O container roda:

- sem capabilities (`--cap-drop ALL`) e com `no-new-privileges`;
- com root filesystem read-only e `/tmp` temporário;
- com o workspace montado em `/workspace`;
- com rede `none` quando `network=false`, senão `bridge`;
- com UID/GID do usuário local para não deixar arquivos root no workspace.

Harnesses extras (Claude Code, Codex, Ollama CLI) devem ser adicionados na
imagem, não no host: o caminho absoluto do binário é reduzido ao basename e
resolvido pelo PATH do container.
