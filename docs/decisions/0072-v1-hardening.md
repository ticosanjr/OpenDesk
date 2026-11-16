# 0072 — Hardening e fechamento da v1.0 (onda 9)

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** as ondas 0–8 entregaram superfícies novas (PWA, webhooks, nós,
  voz, computer-use). Antes de chamar de 1.0 era preciso fechar cabeçalhos,
  prontidão, retenção e documentação.

## Decisão

1. **Cabeçalhos de segurança.** Toda resposta recebe `X-Content-Type-Options`,
   `X-Frame-Options` e `Referrer-Policy`. A PWA servida pelo Axum recebe também
   CSP com `frame-ancestors 'none'`, permitindo apenas recursos locais, dados e
   o WebSocket da própria origem. A API JSON não recebe CSP para não quebrar
   clientes.
2. **Readiness separado de liveness.** `/healthz` continua barato; `/readyz`
   executa `SELECT 1`, não exige autenticação e devolve 503 se o SQLite falhar.
3. **Retenção.** A manutenção de 6 h remove sessões expiradas, sessões
   revogadas há mais de 30 dias e sessões de computer-use encerradas há mais de
   30 dias. Notas de memória expiradas já eram removidas.
4. **Tokens.** Nós usam token próprio com hash; sessões remotas continuam com
   hash, TTL e logout. Nenhum token bruto é persistido.
5. **Versão.** `package.json`, `Cargo.toml`, `Cargo.lock` e `tauri.conf.json`
   passam para `1.0.0`; README, AGENTS, threat model e notas de release são
   atualizados no mesmo corte.
6. **Gate.** O release usa a verificação proporcional; a suíte completa de Rust,
   Vitest, ESLint, `tsc` e goldens visuais foi executada antes do empacotamento.

## Consequências

- A superfície remota tem cabeçalhos defensáveis e um endpoint de prontidão para
  systemd/proxy.
- O banco não cresce indefinidamente com sessões antigas.
- A release 1.0.0 é reproduzível pelo mesmo fluxo local das versões anteriores.
