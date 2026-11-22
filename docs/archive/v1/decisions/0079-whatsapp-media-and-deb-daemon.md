# 0079 — Mídia do WhatsApp e daemon no pacote .deb

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** a Cloud API já recebia mensagens, mas mídia ficava só como
  marcador textual, e o `.deb` instalava apenas o desktop.

## Decisão

1. **Download de mídia.** Imagens, documentos, vídeos, áudios e stickers são
   baixados pela Graph API com o access token, com limite de 25 MB, e salvos em
   `<workspace>/input/whatsapp/<timestamp>-<message-id>.<ext>`.
2. **Extensão segura.** A extensão vem do nome original (somente alfanuméricos,
   até 8 caracteres) ou do MIME; fallback `bin`.
3. **Falha não derruba a mensagem.** Se o download falhar ou exceder o limite, o
   ticket mantém o marcador textual e o erro vai para o log.
4. **`.deb` com daemon.** Após o bundle, o release injeta
   `usr/bin/opendeskd` e a unit de usuário
   `usr/lib/systemd/user/opendeskd.service` no `.deb`, reconstruindo o pacote
   com `dpkg-deb`. Sem `dpkg-deb`, o passo é ignorado.
5. **RPM continua só com o desktop.** Reconstruir RPM exige `rpmbuild`/
   `rpm2cpio`, que não são pré-requisitos do ambiente de release; o binário
   avulso e o AppImage continuam disponíveis.

## Consequências

- Tickets de WhatsApp passam a apontar para arquivos reais no workspace.
- Instalar o `.deb` já deixa o control plane disponível via systemd de usuário.
- O limite de 25 MB evita transformar o banco/workspace em armazenamento de
  mídia sem controle.
