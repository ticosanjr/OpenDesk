# 0098 — SessionContextPacket entre harnesses

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** Plan/Build/Audit podiam usar harnesses diferentes, mas cada um
  recebia apenas o prompt atual. Decisões, diffs e artefatos da etapa anterior
  ficavam só no histórico e o próximo harness recomeçava sem continuidade.

## Decisão

1. `session_context::build` monta um pacote compacto da tarefa:
   fase, plano aprovado, últimas decisões (task messages), diffs recentes,
   artefatos, memória relevante (ACL do agente) e snapshot de orçamento.
2. Limites rígidos evitam inflar o prompt: 12k chars no total, 4k de plano,
   600 por decisão, 1.2k por diff, 16 artefatos e 3k de memória.
3. O pacote é persistido em `session_context_packets` (migração 0053) antes de
   cada dispatch/follow-up. `GET /api/tasks/{id}/session-packet` expõe o pacote
   atual para inspeção/proveniência.
4. `PackContext` ganhou `session_packet`; o context packer injeta a seção
   “Contexto entre harnesses” depois da memória estruturada.
5. Dispatch principal, follow-up e prompt preview usam o mesmo builder. Cron de
   rotina, que não tem tarefa, envia seção vazia.
6. O pacote nunca inclui prompt completo, senha, token ou conteúdo de `.env`.

## Consequências

- Um run de Audit/Ollama vê o plano, as decisões e os diffs do Build/OpenCode.
- O resume de sessão deixa de depender do harness anterior.
- O pacote é reconstruído a cada dispatch; a tabela guarda o último snapshot
  para auditoria e diff futuro.
- Diffs são representados pelo caminho, ação e primeira linha do patch; o
  patch completo continua no artefato/proveniência.
