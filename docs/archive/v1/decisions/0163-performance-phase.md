# 0163 — Fase P: timeline incremental, cache do motor e release com LTO

- **Status:** aceito
- **Data:** 2026-11-21
- **Contexto:** a auditoria pós-1.3.0 apontou custo quadrático no poll da
  timeline, leitura integral do log para devolver 256 KB, processos síncronos
  por request, um N+1 no dashboard de governança e release padrão com perfil
  rápido.

## Decisão

1. **Timeline incremental.** `run_export` mantém um cache por caminho com os
   eventos já parseados e o offset da última linha completa. Polls durante uma
   execução leem apenas os bytes anexados; qualquer redução de tamanho ou
   reescrita força reconstrução. Linhas parciais aguardam o próximo ciclo. O
   cache tem limite de 32 entradas e não guarda segredo nenhum.
2. **`run_log` com seek.** A API lê no máximo 256 KB do fim do arquivo, descarta
   a primeira linha parcial e roda em `spawn_blocking`.
3. **Cache do binário.** `OpenCodeLocator` memoriza a resolução por 60 s,
   invalidando quando o caminho configurado ou o mtime do binário mudam (útil
   para auto-update). `/api/about` resolve em `spawn_blocking`; `rustc` e
   `uname` já eram cacheados desde o ADR 0162.
4. **N+1 de governança.** `monthly_spend_by_agent` agrega o mês inteiro com um
   `GROUP BY agent_name`; o dashboard deixou de rodar uma query por profile.
5. **SQLite.** `journal_mode=WAL` + `synchronous=NORMAL`: consistência
   preservada, um fsync a menos por commit; queda de energia pode perder as
   últimas transações.
6. **Frontend.** O contador de segundos do chat virou o componente
   `ElapsedLabel`, para o timer de 1 s não re-renderizar a página inteira.
7. **Release.** `scripts/release.sh` usa o perfil completo de LTO
   (`lto=true`, `codegen-units=1`) por padrão. `--fast-build` volta ao thin LTO
   para iterações rápidas; `--quality-build` continua aceito como alias.

## Consequências

- O custo de um poll da timeline passa a ser proporcional ao que a run escreveu
  desde o último poll, não ao tamanho total do log.
- O primeiro poll de cada execução ainda lê o arquivo inteiro (necessário para
  montar o estado) e o resultado é reaproveitado.
- Builds de release ficam mais lentos; use `--fast-build` só para testes locais.
- Testes: 340 shell + 291 core (631). Novos: cache de timeline (linha parcial,
  append e reescrita), `read_tail` em boundary e cache do locator invalidado por
  mtime.
