# ADR 0003 — Permissões em runs autônomos

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** o OpenCode V2 pode pausar uma sessão em `permission.asked`
  quando o agente encontra uma ação configurada como `ask`. A fila do OpenDesk
  executa sem uma janela de chat aberta; sem resposta, o job ficaria travado por
  tempo indeterminado.

## Decisão

O runner responde `once` a toda permissão pedida pela sessão e registra a
decisão em `permission_audit` (run, sessão, request, ação, recursos e decisão).
O evento também é transmitido para a UI como `RunStreamChunk` do tipo
`permission`.

## Consequências

- Runs agendados e de Kanban não travam esperando um humano.
- Há um risco de o agente executar ações sensíveis que pediriam confirmação.
  A mitigação atual é a auditoria persistida e o escopo de workspace/agente;
  uma política por workspace (`allow`/`deny`/`ask` + timeout) deve substituir o
  auto-allow na Fase 4.
- O histórico fica consultável mesmo quando a UI não está aberta.
- Se o endpoint de resposta falhar, o job continua sujeito ao timeout do runner
  e à interrupção.
