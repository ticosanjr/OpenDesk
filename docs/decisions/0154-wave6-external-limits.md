# 0154 — Onda 6: limites externos de WhatsApp, voz contínua e GUI

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** a Onda 6 inclui itens que não podem ser validados neste
  ambiente: pareamento de telefone, áudio contínuo e VMs Windows/macOS.

## Decisão

1. **F5** e **F3 (PTT)** foram implementados e testados localmente.
2. **F1/F2 (WhatsApp)** ficam bloqueados até existir:
   - decisão entre whatsmeow e Baileys;
   - telefone de teste e QR;
   - empacotamento do sidecar (binário Go ou runtime Node/Chromium).
3. **F4 (voz contínua)** fica bloqueada até existir:
   - hardware de áudio;
   - VAD/barge-in validados;
   - motores STT/TTS instalados no host de teste.
4. **F6 (computer-use Windows/macOS)** fica bloqueado até existir VM nativa e
   allowlist/isolamento equivalentes ao Linux.
5. Esses itens não são marcados como concluídos e não entram no release como
   "prontos".

## Consequências

- A Onda 6 não pode ser declarada 100% concluída neste ambiente.
- O caminho local (e-mail + PTT) está pronto e coberto por testes.
- A decisão evita simular integrações que não foram pareadas/validadas.
