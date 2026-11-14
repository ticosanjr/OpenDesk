# 0052 — Chat aguarda o turno novo antes de responder

- **Status:** aceito
- **Data:** 2026-11-14
- **Contexto:** o endpoint de prompt do OpenCode é assíncrono. O chat lia
  `/api/session/{id}/message` imediatamente e `parse_assistant` escolhia a
  última mensagem `assistant` de todo o histórico. Quando a resposta nova ainda
  não tinha chegado, o OpenDesk salvava e enviava ao Telegram a resposta
  anterior repetidamente — embora o prompt estivesse sendo entregue ao modelo.

## Decisão

1. Registrar o instante (`since_ms`) antes de enviar o prompt.
2. Fazer polling da lista de mensagens até o turno atual terminar:
   - `idle` criado depois de `since_ms` **e** pelo menos uma mensagem
     `assistant` nova;
   - fallback de estabilidade para versões sem `idle`.
3. `parse_assistant_since(value, since_ms)` considera apenas mensagens
   `assistant` do turno atual, usa o último texto não vazio e soma somente os
   tokens/custo desse turno.
4. Se nenhuma resposta nova aparecer em 5 minutos, retornar erro em vez de
   reutilizar a resposta antiga.
5. O histórico continua sendo mantido pela sessão do OpenCode; o OpenDesk não
   reenvia o histórico inteiro em cada mensagem.

## Consequências

- Duas perguntas seguidas recebem respostas diferentes e correspondentes.
- O ChatPage e o Telegram mostram a resposta do turno certo.
- Um turno sem texto final retorna aviso/erro, nunca uma resposta antiga.
- O polling acrescenta poucas requisições locais enquanto o agente trabalha.
