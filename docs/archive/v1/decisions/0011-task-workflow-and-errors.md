# ADR 0011 — Workflow do Kanban e erros acionáveis

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** mover um card no Kanban só alterava a coluna. Tarefas falhavam sem
  mostrar a causa, e não era possível corrigir agente/modelo/prompt de um card já
  executado.

## Decisão

1. Transições são ações de workflow, não apenas mudança de rótulo:
   - `BACKLOG → PLAN` inicia um plano;
   - `PLAN → BUILD` inicia build (reutilizando o plano existente);
   - `BUILD → REVIEW` inicia auditoria quando a política é `BEST_QUALITY`;
   - `REVIEW → DONE` conclui sem executar;
   - `* → BACKLOG` cancela jobs ativos e limpa a fase.
2. A API expõe `POST /api/tasks/{id}/transition`, que valida engine/budget e
   enfileira o job correspondente.
3. Falhas são persistidas de forma estruturada:
   - `task_runs.error_code`, `error_message`, `error_hint`;
   - `execution_queue.error_message` já existente e agora exposto no DTO.
4. A UI mostra um painel “O que falhou” + “Como resolver” e permite copiar o erro.
5. Tarefas podem ser editadas fora de execução; “Salvar e reexecutar” cria um novo
   run preservando o histórico.
6. O ID de agente é derivado do nome; o campo manual é opcional.
7. Avatares usam presets de emoji e cor, com fallback para emoji livre.

## Consequências

- Mover um card pode consumir budget; por isso há confirmação explícita.
- O histórico de runs permanece imutável.
- Diagnóstico fica disponível mesmo se o log estiver vazio ou o job tiver sido
  retentado.
- Chat e Signal reutilizarão a mesma noção de agente/modelo e o mesmo registro de
  erros.
