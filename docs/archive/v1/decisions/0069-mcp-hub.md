# 0069 — MCP Hub (onda 6)

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o Studio já editava `mcp.servers` por escopo e o OpenCode tinha
  status de runtime, mas não havia um inventário único, catálogo ou teste por
  servidor.

## Decisão

1. **SSOT preservado.** O Hub só lê e escreve `opencode.jsonc` (global e
   workspace) via `jsonc_editor`. Nenhuma tabela paralela de MCP.
2. **Inventário unificado.** `GET /api/mcp-hub/servers` combina escopo global e
   workspace ativo, normaliza V1/V2, identifica transporte (local/remoto) e
   `disabled`.
3. **Saúde em duas camadas.** A validação estática acusa servidor sem `command`
   nem `url`; quando o daemon OpenCode está online, o status real do runtime
   (`connected`/`failed`) é anexado. Teste sob demanda:
   - local: resolve o comando no PATH;
   - remoto: GET com timeout e reporta o HTTP.
4. **Catálogo curado.** Filesystem, Fetch, SQLite, Playwright e Memory graph,
   com templates explícitos e `{{workspace_root}}` substituído na instalação.
   Instalar grava direto no config nativo do workspace.
5. **Ciclo de vida.** Liga/desliga usa `disabled` (V2) e remoção usa o editor
   com backup. Toda mutação passa pela auditoria existente da API local.
6. **UI.** Nova aba Integrações → MCP Hub com inventário, catálogo, teste,
   toggle e remoção.

## Consequências

- Um único lugar mostra o que está configurado no global e no workspace, sem
  duplicar conceitos.
- O catálogo não instala nada silenciosamente: requer ação explícita e mostra o
  comando que será gravado.
- Servidores remotos com autenticação continuam sendo configurados no Studio
  (token/headers), porque o catálogo não carrega segredos.
