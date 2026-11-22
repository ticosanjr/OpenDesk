# 0046 — Tokens e escala tipográfica da UI

- **Status:** aceito
- **Data:** 2026-11-12
- **Contexto:** a UI cresceu com centenas de tamanhos arbitrários
  (`text-[8px]`…`text-[13px]`), dez alturas de controle e pouca separação entre
  corpo, metadado e título. O override de legibilidade em `src/styles.css`
  impedia texto abaixo de 11px, mas tornava a classe mentirosa e mantinha uma
  hierarquia de apenas ~5px entre corpo e título. O Studio chegava a 124
  controles de 28px; confirmações e formulários abriam com a mesma largura de
  64rem porque o `Modal` descartava `max-w-*`.

## Decisão

1. Adotar uma escala semântica em `src/styles.css`:
   - overline 11px, meta 12px, corpo 13.5px, título de card 15px, página 24px,
     KPI 28px.
   - Mono apenas para números, IDs, caminhos e código.
2. Manter o remap legado de `text-[8px]`…`text-[13px]` durante a migração, mas
   ajustá-lo à escala acima. Código novo usa `.type-*` ou `text-sm`.
3. Centralizar superfícies, foco, controles e movimento em tokens
   (`--text-*`, `--radius-card`, `--ease-out`, `--color-surface-*`).
4. Controlar tamanhos em quatro faixas — 28/32/36/40px — com mínimo de 28px
   para alvos interativos. `Button` passa a ter `xs/sm/default/lg/icon`.
5. `Modal` recebe `size` (`sm` 28rem, `md` 42rem, `lg` 56rem, `xl` 64rem). Sem
   `min-width` ou `min-height` forçados; o chamador escolhe o tamanho.
6. `PageBody` limita o conteúdo a 1440px, centralizado. Kanban e Chat continuam
   com largura total e scroll interno.
7. Reduzir ruído: remover overlines de página repetidos, usar uma única badge
   de status por objeto e tooltips em metadados densos.

## Consequências

- A UI fica mais legível sem perder densidade; o corpo sobe para 13.5px e o
  título para 24px.
- Confirmações voltam a ter largura de confirmação; formulários longos usam
  `md/lg/xl`.
- Alturas fixas existentes (`TaskCard`, `Row`, raias) precisam ser auditadas
  para não cortar texto; a regra é altura automática com `min-height`.
- O remap global é dívida temporária: novas telas não devem criar
  `text-[Npx]`; a meta é migrar para `.type-*` e remover o remap.
- `PRODUCT.md` e `DESIGN.md` passam a ser a referência de produto e de sistema
  visual, versionados no repositório.
