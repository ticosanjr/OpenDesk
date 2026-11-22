# ADR 0029 — Receitas declarativas, HITL e Rsync

- **Status:** aceito
- **Data:** 2026-11-07

## Decisão

- Receitas são YAML em `<workspace>/.opencode/recipes/<slug>.yaml`; o SQLite é
  apenas índice/estado (migração 0017).
- O motor executa steps pelo pipeline existente de fila/sessão do OpenCode;
  não há executor paralelo.
- HITL usa `hitl_requests` com token curto one-shot; canais iniciais:
  Desktop UI, E-mail (SMTP/IMAP) e Signal.
- Cinco modalidades: binária, múltipla escolha, aprovação com feedback, edição
  de artefato e proxy de permissão de ferramenta.
- O primeiro canal que responde vence; os demais recebem ACK.
- Timeout aplica `PAUSE_AND_ALERT`, `AUTO_APPROVE` ou `ABORT`.
- Rsync é transporte para pastas locais/remotas:
  - sink `RSYNC_FS` (mapeado para ATOMIC_FS com `transport=rsync`);
  - trigger `RSYNC_FOLDER` (mapeado para FOLDER_WATCH com `transport=rsync`);
  - sem shell, chave SSH no SecretStore, `--delete` opt-in.
- Ferramentas externas (`rsync`, `typst`, `tesseract`, `whisper-cli`, `git`)
  ficam em `~/.local/share/opendesk/tools` e não são embutidas no release.

## Consequências

- Receitas sobrevivem a restart e mantêm estado por step.
- Aprovações podem ser resolvidas pelo celular ou desktop.
- Novos canais implementam traits de HITL sem alterar o motor.
- Sincronização de pastas remotas não exige abrir portas.
