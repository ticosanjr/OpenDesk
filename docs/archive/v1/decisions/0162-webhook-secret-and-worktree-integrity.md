# 0162 — Webhook autenticado, IMAP validado e merge de worktree seguro

- **Status:** aceito
- **Data:** 2026-11-21
- **Contexto:** a auditoria pós-1.3.0 encontrou três falhas de segurança e
  integridade. O webhook HTTP aceitava POST anônimo quando a fonte não tinha
  `secret_ref`, e `auto_execute` podia iniciar um agente. O IMAP desligava a
  validação de certificado quando `tls=false`, apesar de sempre negociar TLS. O
  merge de overlay copiava todo arquivo divergente do worktree, sobrescrevendo
  edições feitas pelo usuário depois do início da execução. O merge git fazia
  `commit` sem pathspec e podia gravar staging alheio.

## Decisão

### Webhook

1. `WEBHOOK_HTTP` sem `secret_ref` não pode ser criado nem atualizado; a
   validação vive no core (`db::io_bus`), então vale para qualquer chamador.
2. `config.secret` em texto puro (formulário de rotina/receita) é movido para o
   `SecretStore` (`io_bus/<ref>`) e substituído por `secret_ref` antes de
   persistir. Rotinas usam `schedule-<id>` e receitas `recipe-<id>-<index>` como
   referência determinística, para o mesmo formulário não gerar segredos órfãos.
3. O handler recusa fonte desativada, fonte sem segredo e assinatura ausente ou
   inválida. A comparação HMAC continua em tempo constante.
4. `trigger_config_json` de rotinas é reescrito sem o segredo em claro.

### IMAP

5. A conexão sempre usa TLS implícito com validação de certificado. O campo
   legado `tls` continua aceito para não quebrar configs, mas não desliga a
   validação. Quem precisa de certificado autoassinado usa
   `allow_invalid_certs: true`, explícito e com aviso em log.

### Worktree

6. O overlay grava `<worktree>.manifest.json` com sha256 por arquivo na
   criação. O merge classifica antes de escrever: aplica só o que a run mudou e
   só quando o workspace ainda está igual à base; caso contrário devolve
   `Conflict` com os paths, sem aplicar nada. Deleções da run também são
   aplicadas quando o alvo não mudou.
7. O merge git exige índice limpo (`git diff --cached` vazio), e o commit de
   merge é criado pelo próprio `git merge --no-ff` (sem `--no-commit` +
   `commit`), para não capturar staging alheio. `git add` e `git commit` passam
   a verificar status.
8. `cleanup` devolve erro quando o diretório ou o branch continuam existindo;
   a API só marca `REMOVED` em caso de sucesso. O manifesto lateral é removido
   no cleanup e no sweep.

### Bugs pontuais

9. `/api/about` reporta `tauri::VERSION` (ou `headless` sem a feature) e cacheia
   `rustc`/`uname`, que eram processos novos por request.
10. O backup pré-migração faz `wal_checkpoint(TRUNCATE)` antes de copiar o
    arquivo, senão páginas commitadas no `-wal` ficavam de fora.

## Consequências

- Fontes webhook antigas sem segredo passam a responder 400 até receberem um
  `secret_ref`. É uma quebra intencional: antes elas aceitavam qualquer origem.
  Rotinas e receitas sem segredo não criam/atualizam a fonte; a sincronização
  registra um aviso e segue com as demais.
- Rotinas e receitas com gatilho WEBHOOK precisam do segredo no formulário
  (ou de um `secret_ref` já existente). O valor vai para o cofre local.
- Worktrees overlay criados antes desta versão não têm manifesto e recusam o
  merge com mensagem explícita; basta descartar e criar outra execução.
- Testes: 339 shell + 289 core (628). Novos: webhook sem segredo/assinatura,
  fonte desativada, política de TLS do IMAP, conflito e deleção no overlay,
  manifesto ausente, staging no merge git, cleanup idempotente e backup com WAL.
