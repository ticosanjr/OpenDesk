# Assistente Pessoal Proativo

Pacote inicial para transformar o OpenDesk em um assistente pessoal:

- **Morning Executive Briefing** — agenda, e-mails prioritários, pendências e
  decisões recomendadas, com aprovação HITL antes de qualquer envio.
- **Briefing sob demanda** — comando `briefing-agora`.
- **Agente e skill** especializados em reduzir ruído, não em despejar a caixa.

## Requisitos

Google Calendar/Gmail, SMTP e Telegram são opcionais: sem eles o briefing
declara explicitamente o que ficou fora.

## Segurança

A receita nasce desligada, o HITL usa `APPROVAL_WITH_FEEDBACK` e nenhum e-mail
ou evento é criado sem aprovação.
