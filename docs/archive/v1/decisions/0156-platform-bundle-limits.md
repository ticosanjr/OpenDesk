# 0156 — B2/B3: limites de empacotamento Windows e macOS

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** MSI/NSIS e DMG exigem máquinas nativas, ferramentas de assinatura
  e certificados que não existem neste ambiente Linux.

## Decisão

1. O `tauri.conf.json` já usa `bundle.targets = "all"`, então cada plataforma
   nativa gera seus instaladores.
2. `docs/platform-builds.md` documenta:
   - MSI/NSIS + WebView2 + Authenticode (`signtool`);
   - DMG + `.app` + hardened runtime + notarização (`notarytool`);
   - inclusão do `opendeskd` como artefato separado e/ou sidecar.
3. `packaging/windows/README.md` e `packaging/launchd/` continuam sendo a base
   para serviço em segundo plano.
4. Nenhum artefato Windows/macOS é declarado pronto sem instalar/atualizar/
   desinstalar em VM/Mac real.

## Consequências

- B2/B3 permanecem **externos** e bloqueiam o release final multiplataforma.
- O release Linux continua válido e testável localmente.
- A assinatura Authenticode e a notarização Apple ficam fora do repositório,
  com segredos na máquina de release.
