# ADR 0042 — Métricas operacionais derivadas, sem nova tabela

## Contexto

O dashboard mostrava custo, runs e fila, mas não respondia perguntas de
operação: qual a latência p95, quanto tempo os jobs esperam na fila, quais
códigos de falha dominam e como os agentes se comparam.

## Decisão

- `GET /api/metrics` calcula métricas sob demanda a partir de `task_runs` e
  `execution_queue`, sem tabela de agregação.
- Janela configurável de 1 a 90 dias.
- Métricas: total/sucesso/falha/cancelado, taxa de sucesso, custo, tokens,
  p50/p95/média de duração, p50/p95/média de espera na fila, falhas por código,
  performance por agente e série diária.
- Percentis são calculados em Rust sobre amostras limitadas pela janela; não
  há aproximação estatística.
- Nova página **Observabilidade** com seletor 7/30/90 dias, gráfico diário,
  falhas e tabela por agente.

## Consequências

- Nenhuma migração ou worker de agregação; o custo é uma consulta por abertura.
- Métricas sempre refletem o banco atual, sem atraso de consolidação.
- Janelas grandes podem custar mais; o limite de 90 dias mantém a consulta
  previsível.
- Uma futura tabela materializada pode ser adicionada se o volume exigir, sem
  mudar o contrato da API.
