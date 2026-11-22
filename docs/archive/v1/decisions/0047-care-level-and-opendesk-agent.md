# 0047 — Nível de cuidado e agente OpenDesk

- **Status:** aceito
- **Data:** 2026-11-13
- **Contexto:** o agente `build` é deliberadamente rigoroso e, em tarefas
  comuns, explora arquivos e faz validações demais. Ao mesmo tempo, tarefas
  simples, instalações e operações de catálogo precisam de um caminho direto e
  previsível, sem transformar todas as execuções em Plan/Audit.

## Decisão

1. Adicionar `tasks.care_level` (`quick`, `standard`, `careful`, default
   `standard`) na migração 0034. O campo é editável no formulário da tarefa e
   entra no fingerprint de deduplicação.
2. O dispatch injeta um bloco de instruções no prompt conforme o nível:
   - `quick`: sem plano formal/auditoria, uma validação objetiva;
   - `standard`: checklist curto e uma validação;
   - `careful`: revisão de premissas, riscos e validação completa.
3. Quando o cuidado é `quick`, a fase é `build`, não há agente escolhido por
   usuário/regra e o arquivo `.opencode/agents/opendesk.md` existe no workspace
   ou no escopo global, o dispatch usa o agente `opendesk`; caso contrário
   mantém a decisão do roteador (normalmente `build`). O override acontece no
   `plan_decision`, antes do budget guard, para que orçamento e o banner da UI
   usem o agente final.
4. O pacote base `controle-opendesk` 1.1.0 inclui `opendesk.md`: agente direto
   para tarefas comuns, instalações e rotinas, com instrução explícita para
   encaminhar código ao `build`.
5. Workspaces existentes recebem o agente via **Pacotes → Atualizar todos**.
   Até a atualização, o cuidado rápido continua funcionando com o `build`.

## Consequências

- Tarefas comuns deixam de pagar o custo de um fluxo de engenharia completo.
- Tarefas de código continuam no `build`; o nível `quick` não altera a fase
  decidida pelo roteador.
- O usuário pode sobrescrever o agente manualmente; a decisão manual vence.
- A escolha `quick` não é um atalho inseguro para instalações: tarefas de
  instalação usam `standard` e continuam rastreáveis em Execuções.
