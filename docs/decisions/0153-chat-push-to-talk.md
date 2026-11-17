# 0153 — F3: push-to-talk no Chat

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** a tela de Configurações já testava STT/TTS, mas a conversa não
  tinha entrada por voz nem leitura das respostas.

## Decisão

1. O compositor do Chat ganhou um botão de microfone:
   - grava com `MediaRecorder`;
   - envia o áudio para `/api/voice/transcribe`;
   - acrescenta o texto transcrito ao prompt.
2. Respostas do assistente têm um botão de alto-falante:
   - chama `/api/voice/speak`;
   - toca o áudio retornado em base64.
3. Estados de gravação/transcrição/fala aparecem no próprio botão; erros usam
   toast acionável.
4. O backend continua usando motores locais configurados em Configurações →
   Voz; nenhum modelo é baixado automaticamente.

## Consequências

- F3 (PTT) fica utilizável no Chat quando há microfone e STT/TTS instalados.
- F4 (voz contínua/VAD/barge-in) não foi implementada; depende de áudio real,
  VAD e validação com hardware, e permanece externa.
