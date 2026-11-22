# ADR 0037 — Pergunta e consequências persistentes no HITL

## Contexto

Os canais (desktop, e-mail, Signal) recebiam a mensagem renderizada do
checkpoint, mas `hitl_requests` guardava apenas ids, tipo, opções, timeout e
expiração. O Attention Card não tinha como mostrar a pergunta e exibia botões
genéricos de aprovar/rejeitar, sem explicar o efeito da decisão.

## Decisão

- `hitl_requests` passa a guardar `title`, `question_markdown` e `context_json`.
- O contexto inclui receita, run, tarefa, checkpoint, próximo step, canais,
  anexos e política de timeout.
- A UI apresenta a pergunta como elemento principal, três consequências
  explícitas (aprovar, rejeitar, expirar) e ações específicas por tipo:
  aprovação binária, aprovação com ajustes, múltipla escolha, ciência e revisão
  de artefato.
- Aprovação de plano mostra um trecho do plano e o botão “Aprovar e iniciar
  Build”. Permissões explicam o alcance de “uma vez”, “nesta tarefa” e “negar”.

## Consequências

- O usuário decide com contexto, não por adivinhação.
- O conteúdo da pergunta fica disponível para futuras integrações e auditoria.
- Nada do fluxo de execução muda: canais e engine continuam usando a mesma
  mensagem renderizada.
