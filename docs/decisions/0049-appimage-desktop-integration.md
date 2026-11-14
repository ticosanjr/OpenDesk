# 0049 — Integração do AppImage com o menu e a barra de tarefas

- **Status:** aceito
- **Data:** 2026-11-14
- **Contexto:** o AppImage executado diretamente não tem entrada `.desktop`
  instalada. O `_NET_WM_ICON` da janela contém o logo correto, mas ambientes
  como o Cinnamon resolvem o ícone da lista de janelas pelo `.desktop`
  associado ao `WM_CLASS`; sem ele, mostram o ícone genérico de executável
  (uma engrenagem).

## Decisão

1. No Linux, quando `APPIMAGE`/`APPDIR` estão presentes, criar ou atualizar
   `~/.local/share/applications/opendesk.desktop` no startup.
2. Copiar o ícone de `$APPDIR/.DirIcon` (ou `OpenDesk.png`/`opendesk.png`) para
   o diretório de dados do app e usar caminho absoluto em `Icon=`.
3. Usar `StartupWMClass=Opendesk` para casar com a classe real da janela
   (`WM_CLASS = "opendesk", "Opendesk"`) e `Exec="<AppImage>" %U`.
4. Escrever de forma idempotente: regravar apenas se o conteúdo mudou e
   atualizar o caminho caso o AppImage seja movido. Rodar
   `update-desktop-database` em best-effort.
5. Expor `POST /api/about/desktop-integration` e um botão **Reintegrar atalho**
   em Configurações → Sobre para reparo manual.
6. Não agir quando o app roda de `.deb`/`.rpm` ou em modo `--headless`: esses
   pacotes já instalam o `.desktop`, e um servidor não deve escrever em
   `~/.local/share/applications`.

## Consequências

- O AppImage passa a aparecer no menu, no dock e na barra de tarefas com o mesmo
  ícone do tray e da interface.
- Um atalho já fixado antes da correção pode precisar ser removido e fixado de
  novo, porque o ambiente guarda a referência antiga.
- A escrita é limitada ao diretório do usuário e pode ser refeita a qualquer
  momento pelo botão de reparo.
