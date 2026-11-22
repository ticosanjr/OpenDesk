# 0055 — HITL 2.0 com estruturas explícitas e modal global

- **Status:** aceito
- **Data:** 2026-11-14
- **Contexto:** o HITL tinha dois modelos incompatíveis. Receitas usavam
  `BINARY_GATE`, `MULTI_CHOICE`, `APPROVAL_WITH_FEEDBACK` e variantes; tarefas
  usavam apenas opções livres + texto. O agente não escolhia a estrutura, e o
  usuário só encontrava a pergunta no Dashboard ou no Kanban.

## Decisão

1. **Quatro estruturas canônicas**: `APPROVAL`, `SINGLE_CHOICE`,
   `MULTI_CHOICE` e `TEXT`.
   - `APPROVAL`: aprovar/negar, com payload opcional.
   - `SINGLE_CHOICE` / `MULTI_CHOICE`: opções obrigatórias, com payload
     opcional junto da escolha.
   - `TEXT`: resposta livre obrigatória; negar encerra sem responder.
2. **Compatibilidade**: nomes legados de receita são mapeados na entrada
   (`BINARY_GATE`, `APPROVAL_WITH_FEEDBACK`, `ARTIFACT_EDIT_HANDOFF`,
   `TOOL_PERMISSION_PROXY`, `ACKNOWLEDGE`, `ARTIFACT_REVIEW` → `APPROVAL`).
   Interações antigas sem `kind` derivam a estrutura das opções existentes.
3. **O agente escolhe**: `ask_user` ganhou `kind` com o enum das quatro
   estruturas. Se omitido, a estrutura é derivada das `options` para não quebrar
   chamadas antigas.
4. **Resposta única**: `POST /api/interactions/{id}/resolve` aceita
   `{ decision, selected, text }`. `decision` é `approve`, `deny` ou `answer`;
   o payload é o `text`. O `answer_json` guarda os três campos.
5. **Canais**: Telegram ganhou botões Aprovar/Negar e payload por
   `#código aprovar <texto>`; Signal ganhou notificação e resposta por
   `#código`; o Desktop ganhou um modal global montado no `AppShell`, que abre
   em qualquer página quando há decisão pendente, com notificação nativa.
6. **Receitas**: os mesmos quatro nomes são aceitos no YAML; a UI mostra a
   consequência e as opções com a mesma linguagem.
7. Migração 0039 adiciona `task_interactions.kind` e preenche as linhas antigas.

## Consequências

- Um único vocabulário para todo HITL, sem perder receitas existentes.
- Toda decisão pode carregar uma explicação curta (payload), que o agente
  recebe junto da escolha.
- O usuário não precisa estar no Dashboard para responder: o modal aparece
  onde estiver, e os canais remotos continuam funcionando.
- Aprovação exige decisão explícita; escolhas e texto validam o payload no
  backend, não apenas na UI.
