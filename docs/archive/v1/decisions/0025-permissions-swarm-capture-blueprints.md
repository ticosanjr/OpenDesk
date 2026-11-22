# ADR 0025 — Permissões interativas, swarm, captura rápida e blueprints

- **Status:** aceito
- **Data:** 2026-11-07
- **Contexto:** o engine já pausava em `permission.asked`, mas o OpenDesk
  respondia `once` automaticamente. O usuário não via o agente bloqueado, não
  podia negar uma ação, e faltavam compactação, captura global, rollback de
  artefato, comparação A/B e times iniciais.

## Decisão

### Permissão interativa
- `permission_requests` guarda cada pedido pendente com run, tarefa, workspace,
  sessão, ação e recursos.
- O runner registra o pedido, publica `PermissionRequested` e espera a decisão
  humana (até 15 min, cancelável pelo lease).
- A resposta aceita `once`, `always` e `reject` e é enviada ao engine como
  `{ "decision": ... }`.
- O Attention Card mostra os pedidos com **Permitir**, **Sempre nesta tarefa**
  e **Negar**, além de aprovação de plano, bloqueios e falhas.

### Estados de agente
- `/api/agents/states` deriva `BLOCKED`, `WORKING`, `DONE` e `IDLE`.
- O Dashboard exibe o estado em cada agente.

### Contexto
- `/api/tasks/{id}/context` lê a janela de 128k do último run.
- `/api/tasks/{id}/compact` chama a compactação durável do engine.
- O runner compacta automaticamente acima de 100k tokens de entrada.

### Swarm A/B e fork por run
- `/api/tasks/{id}/swarm` cria cópias com modelo fixo e pasta de saída própria.
- A UI permite rodar 2–4 modelos e marcar o vencedor como Done.
- `/api/runs/{id}/fork` copia os artefatos gerados até o run para uma nova
  pasta e cria um card independente.

### Quick Capture
- `Ctrl/Cmd+Shift+Space` abre uma janela flutuante.
- Texto, links e imagens coladas viram tarefa em Backlog; imagens são salvas em
  `input/captures/YYYY-MM/`.

### Rollback e entregáveis
- `POST /api/artifacts/{id}/rollback` remove arquivos criados ou reaplica o
  texto anterior de edições. Se o arquivo mudou, recusa o rollback.
- Entregáveis fixados aparecem no Dashboard e nos cards do Kanban.

### Blueprints
- `GET /api/blueprints` e `POST /api/blueprints/{id}/apply` criam agentes e
  skills iniciais em `.opencode/` sem sobrescrever arquivos existentes.

## Consequências

- Nenhuma ação sensível do agente é aprovada sem o usuário quando o pedido é
  interativo.
- Falhas de provedor têm retry em outro modelo a partir do Attention Card.
- O contexto não cresce indefinidamente e a continuidade da sessão é mantida.
- Trabalho descartado e comparações ficam isolados por pasta.
- Workspaces novos já nascem com um time coerente.
