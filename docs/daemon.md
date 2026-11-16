# opendeskd — control plane headless

O `opendeskd` usa o mesmo estado, SQLite, fila, scheduler e API Axum do app
desktop, sem abrir janela ou tray.

## Configuração

| Variável | Efeito |
|---|---|
| `OPENDESK_API_HOST` | Host de bind; padrão `127.0.0.1` |
| `OPENDESK_API_PORT` | Porta fixa; padrão portas dinâmicas |
| `OPENDESK_WEB_DIR` | Diretório do build Vite (`dist`) para servir a PWA |
| `OPENDESK_DATA_DIR` | Diretório de dados/SQLite |
| `OPENDESK_SKIP_DAEMON=1` | Mantém o motor OpenCode offline |

O token da API fica em `~/.local/share/opendesk/api.json` (0600) e é o mesmo
usado pelo PWA no primeiro login.

## Linux (systemd user)

```bash
mkdir -p ~/.local/bin ~/.local/share/opendesk/web
cp target/release/opendeskd ~/.local/bin/
cp -r dist/* ~/.local/share/opendesk/web/
cp packaging/systemd/opendeskd.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now opendeskd
```

## macOS (launchd)

Copie `opendeskd` para `/usr/local/bin`, ajuste o plist e carregue com
`launchctl load -w packaging/launchd/com.opendesk.opendeskd.plist`.

## Acesso remoto

Mantenha `OPENDESK_API_HOST=127.0.0.1` e exponha via Tailscale, WireGuard ou um
reverse proxy HTTPS. O PWA pede o token da API no primeiro acesso; passkeys e
sessões persistentes entram na onda de autenticação remota.

## PWA

Com `OPENDESK_WEB_DIR` apontando para `dist`, o Axum serve a SPA, o manifest e o
service worker. O Web Push/VAPID é opcional: ative em Configurações →
Notificações no PWA (exige HTTPS ou localhost) e use “Testar” para validar o
caminho até o serviço de push.
