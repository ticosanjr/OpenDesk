# 0103 — Allowlist de janela/app no computer-use

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** mesmo com display isolado, uma sessão podia enviar entrada para
  qualquer janela/aplicativo. Faltava limitar o alvo do computer-use.

## Decisão

1. `WindowAllowlist` é persistida em `settings` (`computer_allowlist`):
   `enabled`, `windows` e `apps`, normalizados, sem vazios, até 50 itens.
2. Correspondência:
   - janela: substring case-insensitive do título ativo;
   - app: substring case-insensitive do processo (`/proc/<pid>/comm`).
3. Quando habilitada, toda captura e toda ação consultam a janela ativa via
   `xdotool` antes de executar. Falha ao ler a janela ou alvo fora da lista
   resulta em `Conflict` — falha fechada.
4. Backend Wayland sem display isolado é rejeitado com orientação para usar uma
   sessão Xvfb.
5. API `GET/PUT /api/computer/allowlist` e controles na tela de Sandbox/
   Computer-use (checkbox, títulos e processos separados por vírgula).
6. Sessões isoladas mostram `display`/`isolated` na lista; a allowlist vale para
   entrada e screenshot.

## Consequências

- Um agente só interage com a janela/aplicativo permitido.
- A verificação é por título/processo, não por identidade criptográfica da
  janela; é uma barreira operacional, não um sandbox de compositor.
- Equivalente Windows/macOS segue pendente em REQ-15.
