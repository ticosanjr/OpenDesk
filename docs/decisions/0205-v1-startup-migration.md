# 0205 — Abertura com banco V1: diálogo, export e relançamento

- **Status:** aceito
- **Data:** 2026-11-22
- **Contexto:** a 1.4.0 recusava abrir um banco V1 dentro do `setup`, depois de
  a janela já ter sido criada. O usuário via o ícone piscar e o processo sair,
  sem tray e sem explicação. As notas mandavam usar `opendesk-cli`, que o
  AppImage não empacota.

## Decisão

1. **Pré-voo antes de qualquer efeito:** no `ProbeOutcome::Embedded`, antes de
   `ServerLock`, API, janela ou tray, `legacy_version` abre o SQLite em
   somente-leitura e detecta `_sqlx_migrations` sem `v2_meta`.
2. **Diálogo nativo** com “Exportar e continuar” / “Sair”. A janela principal
   nasce `visible: false`; ela só aparece depois de `AppState::initialize`,
   control plane, API e tray.
3. **Migração automática consentida:**
   - exporta workspaces, receitas e settings para
     `<data>/v1-export-<timestamp>/` (JSON + manifesto SHA256, sem segredos);
   - renomeia `opendesk.db` para `opendesk.db.v1-<timestamp>` e move
     `-wal`/`-shm` junto;
   - relança o app. No AppImage, relança o arquivo `$APPIMAGE` via `setsid`
     (reexecutar o binário interno enquanto o mount desmonta mataria o novo
     processo); no pacote instalado, usa `current_exe`.
4. **Comando no binário desktop:** `opendesk migrate-v1-export [--db] [--out]
   [--format]`, usando o mesmo `opendesk_storage::legacy`.
5. **Erros de startup** (qualquer um, não só V1) mostram um diálogo e saem com
   código 1, em vez de sumir sem explicação.
6. **Empacotamento:** o release sempre reconstrói `opendeskd` e limpa RPMs
   antigos do daemon antes de gerar o novo.

## Consequências

- Um usuário com banco V1 consegue abrir a 1.4.1, preservar os dados antigos e
  começar no V2 com um clique.
- O banco V1 nunca é aberto pelo schema V2; o backup fica ao lado.
- Testes: detecção V1, export JSON/YAML, recusa V2, migração com backup e
  criação do V2; export validado contra uma cópia de banco real.
- Limitação: o diálogo é GUI; a lógica de detecção/export/migração é testada
  separadamente e o fluxo visual deve ser confirmado numa sessão desktop.
