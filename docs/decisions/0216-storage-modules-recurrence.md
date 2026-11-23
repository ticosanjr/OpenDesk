# 0216 — Módulos reais no storage e `Recurrence` consolidado

- **Status:** aceito
- **Data:** 2026-11-23
- **Contexto:** `tasks`, `schedules`, `calendar` e `memory` eram arquivos de 5
  linhas com dois `include!("*_part*.rs")`, o que quebrava ferramentas de
  análise, impedia `mod` real e escondia dependências entre tipos e queries.
  Havia ainda o modelo `Recurrence` em `queue_scheduler/recurrence.rs`, separado
  da resolução `next_occurrence` em `scheduler/recurrence.rs`.

## Decisão

1. Cada grupo virou um diretório de módulo:
   - `repos/tasks/{mod,model,mutations,tests}.rs`
   - `repos/schedules/{mod,model,mutations,tests}.rs`
   - `repos/calendar/{mod,sources,events,tests}.rs`
   - `repos/memory/{mod,model,notes,tests}.rs`
   Os arquivos `*_part1/2.rs` foram apagados. Helpers e constantes usados por
   dois módulos ficaram `pub(super)`; a API pública (`repos::tasks::*` etc.) não
   mudou.
2. Testes deixaram de usar `include!` nesses quatro grupos e viraram
   `#[cfg(test)] mod tests;`.
3. O modelo `Recurrence`/`parse_recurrence` foi movido para
   `engine::scheduler::recurrence`, junto de `next_occurrence`.
   `queue_scheduler::recurrence` virou `pub use crate::scheduler::recurrence`,
   preservando todos os caminhos existentes. Não havia duas implementações de
   `next_occurrence`; o arquivo antigo era o modelo.
4. `chat`, `chat_turns`, `interactions` e `task_messages` continuam como
   fachadas finas sobre `repos/conversations/*`: não contêm SQL nem structs
   duplicadas; apenas preservam a API e a conversão `RunUsage → ChatTurnUsage`.
   Eliminar as fachadas só adicionaria churn em callers sem ganho.
5. `include!` de testes em outros crates permanece permitido; a regra do gate é
   banir `include!` apenas em código de produção.

## Consequências

- Ferramentas de análise e rustfmt enxergam cada submódulo real.
- Nenhum Símbolo público do storage mudou.
- `Recurrence` e `next_occurrence` vivem no mesmo módulo, sem reexport cruzado
  desnecessário.
- Testes de storage seguem em 87; engine em 134.
