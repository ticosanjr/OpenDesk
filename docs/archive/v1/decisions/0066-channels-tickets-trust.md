# 0066 — Canais, tickets e confiança (onda 3)

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** Telegram e Signal já conversavam com o OpenDesk, mas cada canal
  tinha regras próprias e não havia um lugar único para mensagens que não
  deveriam virar tarefa. WhatsApp precisava nascer como estrutura, sem conector.

## Decisão

1. **Rota por identidade.** `channel_routes` mapeia `(canal, identidade)` para
   agente, perfil de permissão e `trust_level` (`trusted`, `internal`,
   `untrusted`). `identity = "*"` é o padrão do canal; a rota exata vence.
2. **Ticket primeiro.** Toda mensagem inbound vira ticket idempotente por
   `(workspace, canal, message_id)`. Isso preserva o histórico mesmo quando a
   mensagem é recusada como tarefa.
3. **Confiança decide execução.**
   - `untrusted`: só ticket, nenhuma tarefa.
   - `internal`: cria tarefa retida (`held`) para revisão humana.
   - `trusted`: cria tarefa em `TODO` (sem auto-dispatch).
   O perfil `read_only` sempre retém; `full` nunca retém.
4. **Webhook autenticado por canal.** O token é gerado em Integrações → Canais,
   guardado no `SecretStore` e exigido no header
   `x-opendesk-channel-token`. Sem token, o endpoint recusa.
5. **WhatsApp estrutural.** Configuração de provider/número/notas e rota já
   funcionam; o conector real fica para a onda de canais externos.
6. **Inbox visível.** A mesma aba lista tickets e permite triagem
   (`OPEN`/`TRIAGED`/`IN_PROGRESS`/`RESOLVED`/`DISMISSED`).

## Consequências

- Um único webhook genérico serve WhatsApp, email, web e integrações futuras.
- Mensagens não confiáveis não conseguem criar tarefas executáveis.
- Telegram/Signal mantêm seus bridges nativos; a rota de confiança é o ponto de
  convergência para a próxima onda.
