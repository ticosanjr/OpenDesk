# 0084 — Ciclo de vida e consolidação de memória

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** notas expiradas eram apagadas e duplicatas/baixa confiança
  degradavam o contexto ao longo de meses.

## Decisão

1. **Arquivo em vez de exclusão.** `memory_notes.archived_at` marca notas
   expiradas ou consolidadas; busca/listagem/contexto ignoram arquivadas.
2. **Decaimento.** Notas `observed`/`inferred` sem atualização por 30 dias
   perdem 0,05 de confiança; abaixo de 0,1 são arquivadas. `verified` não decai.
3. **Deduplicação exata.** Mesmo workspace/namespace/título/corpo mantém a nota
   de maior confiança e arquiva as demais.
4. **Contadores.** `access_count`/`last_accessed_at` ficam disponíveis para
   heurísticas futuras; não são atualizados em toda leitura para não criar
   contenção.
5. **Manutenção.** A rotina de 6 h chama `memory::consolidate`.

## Consequências

- O contexto não cresce indefinidamente e mantém trilha auditável.
- Consolidação semântica por embeddings continua pendente; esta é exata e barata.
