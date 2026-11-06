# ADR 0009 — Resolução e fallback do motor OpenCode

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** o OpenDesk AppImage herda um `PATH` sem `~/.opencode/bin`.
  Executar `Command::new("opencode")` falhava com `ENOENT`, mesmo com o OpenCode
  instalado e saudável. O app ficava offline sem explicar como corrigir.

## Decisão

1. Um `OpenCodeLocator` resolve o binário nesta ordem:
   caminho salvo em `app_settings.opencode_bin_path` → `OPENCODE_BIN` /
   `OPENDESK_OPENCODE_BIN` → `PATH` → caminhos conhecidos por plataforma.
2. Todo candidato é validado com `opencode --version`; apenas V2 é aceito.
3. O caminho validado é persistido, de modo que o AppImage continue funcionando
   nos próximos inícios.
4. Se nenhum binário servir, o supervisor tenta anexar ao serviço compartilhado
   do OpenCode lendo `~/.local/state/opencode/service.json` (ou
   `~/.config/opencode/service.json`) e validando `/api/info` com Basic auth.
5. O status do daemon expõe `mode` (`dedicated`, `shared`, `offline`) e
   `binary_path`, além do motivo da falha.
6. Dispatch, aprovação de plano e “executar agora” fazem preflight e retornam
   503 com instrução quando o motor está offline.

## Consequências

- O app funciona sem `opencode` no `PATH`, sem symlink e sem serviço público.
- O usuário pode escolher o binário manualmente em Configurações.
- Um serviço compartilhado existente é reutilizado quando o spawn falha.
- Nenhuma tela fica “offline e morta”: há motivo, caminhos testados e ação.
- O modo headless de testes (`OPENDESK_SKIP_DAEMON`) ignora o preflight para
  permitir testar a fila sem um motor real.
