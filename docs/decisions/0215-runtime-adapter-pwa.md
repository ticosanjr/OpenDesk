# 0215 — Adaptador único de runtime (Tauri vs PWA)

- **Status:** aceito
- **Data:** 2026-11-23
- **Contexto:** componentes importavam `@tauri-apps/plugin-opener` diretamente e
  testavam `window.__TAURI_INTERNALS__` cada um à sua maneira. Em modo PWA essas
  chamadas lançavam exceções não tratadas (abrir link, abrir pasta, fechar o
  Quick Capture), quebrando a experiência no navegador.

## Decisão

1. `src/lib/runtime-env.ts` é a única porta:
   - `isTauriRuntime()`;
   - `openExternalUrl(url)` com validação `http(s)` e bloqueio fail-closed de
     `javascript:`, `file:` e outros esquemas;
   - `openLocalPath(path)` com fallback `{ ok: false, fallback }` no PWA;
   - `openWorkspacePath(path)` usando o comando Rust allowlisted;
   - `hideWindowIfTauri()` para o Quick Capture.
2. Componentes/páginas não importam mais `@tauri-apps/plugin-opener` nem testam
   `__TAURI_INTERNALS__` manualmente. O PWA usa `window.open` para links e exibe
   o caminho local em um toast quando não pode abrir.
3. `src/lib/open-path.ts` permanece como reexport de compatibilidade.
4. Guias de integração (Google OAuth, extensão Browse) passam pelo mesmo
   adaptador; erros de popup bloqueado são propagados para a UI.

## Consequências

- `rg "@tauri-apps/plugin-opener" src/components src/pages` fica vazio.
- `javascript:` nunca chega a `window.open` nem ao plugin Tauri.
- Testes cobrem browser, Tauri mockado, esquema inválido (browser e desktop),
  popup bloqueado, fallback local e allowlist de workspace.
- Nenhuma mudança de DOM/layout; goldens continuam válidos.
