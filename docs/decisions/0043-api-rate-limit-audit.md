# ADR 0043 — Rate limit e auditoria da API local

## Contexto

A API local usa Bearer token em arquivo 0600 e CORS restrito, mas qualquer
processo do mesmo usuário pode ler o token e disparar chamadas. Faltavam defesas
contra rajadas/loops e um registro consultável de operações sensíveis.

## Decisão

- **Rate limit em memória** por classe de rota, janela fixa de 60s:
  - leitura: 900/min;
  - escrita: 300/min;
  - sensível: 120/min (backups, Browse, Studio, MCP, tasks, receitas, rotinas,
    integrações, settings, governança e aprendizado).
- Limites configuráveis por `OPENDESK_RATE_LIMIT_READ`,
  `OPENDESK_RATE_LIMIT_WRITE` e `OPENDESK_RATE_LIMIT_SENSITIVE`.
- Resposta `429` com `Retry-After` e código `rate_limited`.
- **Auditoria em SQLite** (`api_audit`): ação, método, rota, status, duração,
  workspace opcional e timestamp. Corpo, prompts, headers e credenciais nunca
  são gravados.
- São auditadas mutações e leituras sensíveis (`/download`, `/export`).
- Retenção de 90 dias no maintenance.
- UI: **Configurações → Auditoria**, com filtros e limites vigentes.

## Consequências

- Rajadas acidentais e loops de agentes não derrubam a API local.
- Operações sensíveis ficam rastreáveis sem vazar conteúdo.
- Limites são globais por processo, não por IP/usuário; suficiente para o modelo
  local single-user.
- A auditoria é best-effort e assíncrona: não bloqueia a resposta nem falha a
  operação original.
