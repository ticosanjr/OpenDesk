# 0157 — Piso de cobertura após as ondas 1–6

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** o usuário aceitou não bloquear o release na meta de 80%. Depois
  das ondas A–G, a cobertura medida caiu porque várias integrações novas são
  dependentes de ambiente (MCP, voz, TLS, sync, e-mail, loops).

## Decisão

1. O gate passa a exigir **53,8% totais** e **62,7% do core** — cerca de 0,5
   ponto abaixo do medido (54,38% e 63,24%).
2. O ratchet continua: novos módulos não podem derrubar esses pisos.
3. A meta de 80% deixa de ser critério de H4; o piso acordado é o critério.
4. A cobertura de integrações externas continua opt-in e não conta como
   desculpa para baixar ainda mais o piso.

## Consequências

- `scripts/gate.sh` fica verde com o estado atual.
- H2 pode ser declarado concluído no piso acordado, não em 80%.
- O relatório completo fica em `docs/coverage-plan.md`.
