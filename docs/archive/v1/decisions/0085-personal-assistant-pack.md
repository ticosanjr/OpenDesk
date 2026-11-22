# 0085 — Pack inicial de assistente pessoal

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o catálogo tinha packs de e-mail, viagens e casa, mas faltava um
  fluxo proativo de briefing matinal com aprovação antes de qualquer envio.

## Decisão

1. **Pack `assistente-pessoal`** com agente, skill, comando e a receita
   `morning-briefing`.
2. **Briefing matinal**: coleta agenda/e-mails/pendências, passa por
   `APPROVAL_WITH_FEEDBACK` e só então gera a versão final.
3. **Sem efeitos externos sem HITL**: a receita nasce desligada e a primeira
   versão apenas escreve em `briefing/`.
4. **Integrações opcionais**: Google Calendar/Gmail, SMTP e Telegram; o que
   faltar é declarado no próprio briefing.
5. **Catálogo**: entrada adicionada em `starter-packs/catalog.json`; exemplos
   apontam para a receita e o comando.

## Consequências

- O usuário instala em 1 clique e ganha um fluxo proativo diário.
- Inbox Triage & Drafts e Personal Life & Travel Ops continuam parcialmente
  cobertos pelos packs `email-zero` e `viagens-planner`; um pack unificado fica
  para a próxima fatia.
