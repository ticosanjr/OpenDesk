# ADR 0004 — Proveniência com fallback por tool calls

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** a fonte autoritativa de diff do OpenCode V2 é
  `GET /api/session/:id/diff`. Em workspaces sem snapshot/VCS, o endpoint pode
  retornar uma lista vazia mesmo quando o agente criou arquivos com a tool
  `write` (observado no teste live com `deepseek/deepseek-flash`).

## Decisão

O `stream_interceptor` combina duas fontes:

1. **Primária:** `session/:id/diff`, que fornece caminho, patch e status.
2. **Fallback:** as tool calls `write`, `edit` e `patch` nas mensagens finais da
   sessão. O OpenDesk extrai `path`/`filePath`, monta um patch unificado mínimo
   (`CREATED` para write, `MODIFIED` para edit/patch) e usa o arquivo real para
   tamanho e sha256.

Artefatos já vistos pelo diff não são duplicados.

## Consequências

- Todo arquivo criado por `write` aparece na aba Artefatos mesmo sem diff do
  servidor.
- O patch de fallback é sintético e pode ser menos completo que o diff real,
  especialmente para bash e patches complexos.
- `bash` que cria arquivos fora das tools de escrita ainda depende do diff ou de
  uma varredura de sistema de arquivos; isso fica como evolução futura.
- A UI deve indicar que o patch é derivado quando o diff autoritativo não
  existir (campo de origem pode ser adicionado em uma migração futura).
