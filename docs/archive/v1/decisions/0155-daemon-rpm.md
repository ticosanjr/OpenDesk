# 0155 — B1: RPM do daemon com systemd user unit

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** o `.deb` já recebia `opendeskd` e a unit, mas o RPM do Tauri
  continuava apenas com o desktop.

## Decisão

1. O daemon ganha um RPM próprio, `opendeskd`, em vez de mutar o RPM gerado pelo
   Tauri (que exigiria reconstruir o spec original).
2. `packaging/rpm/opendeskd.spec` empacota:
   - `/usr/bin/opendeskd`;
   - `/usr/lib/systemd/user/opendeskd.service`.
3. `scripts/build-daemon-rpm.sh` monta um `_topdir` temporário e usa `rpmbuild`.
4. `scripts/release.sh` chama o script no Linux quando `rpmbuild` existe; sem
   ele, avisa e segue.
5. Validação local: RPM `opendeskd-1.2.0-1.x86_64` inspecionado com
   `rpm2cpio | cpio -t` contendo os dois caminhos.

## Consequências

- Instalação headless: `dnf install ./opendeskd-*.rpm` e
  `systemctl --user enable --now opendeskd`.
- O RPM do desktop continua separado; não há conflito de arquivos.
- A validação foi feita com `rpmbuild` extraído localmente; em uma máquina
  Fedora/RHEL basta o pacote `rpm`.
