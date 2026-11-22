# 0078 — OpenCode em worker remoto

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o worker remoto só executava harnesses CLI. Jobs `opencode`
  falhavam, o que deixava o multi-node incompleto para o motor principal.

## Decisão

1. **Daemon local no nó.** O worker aceita `--opencode-url` e
   `--opencode-password` (ou `OPENDESK_OPENCODE_URL/PASSWORD`) e usa o
   `RestClient` existente para criar sessão, enviar prompt e consumir SSE.
2. **Mesmo contrato.** Texto, sucesso/falha, idle e diffs são convertidos para o
   mesmo formato de `complete` usado pelos harnesses CLI; a central registra
   artefatos e fecha o run.
3. **Sem daemon, falha explícita.** Sem URL/senha, o job `opencode` é recusado
   com mensagem clara, em vez de tentar executar o protocolo OpenCode via CLI.
4. **Limites.** 30 minutos de execução e 20 minutos sem eventos; em timeout o
   worker chama `interrupt`.
5. **Segurança.** A senha deve vir de ambiente; `--opencode-password` continua
   aceito para uso manual, mas aparece no histórico do shell.

## Consequências

- Qualquer nó com OpenCode local pode executar jobs `opencode` e reportar diffs.
- A central não precisa expor a senha do daemon; cada nó tem a sua.
