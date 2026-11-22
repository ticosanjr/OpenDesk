# 0065 — Harnesses plugáveis e seleção no DSH (onda 2)

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** a onda 0 definiu o contrato `AgentRuntimeAdapter`, mas apenas o
  OpenCode executava. A v1.0 precisa rodar Claude Code, Codex, Pi e Hermes sem
  duplicar fila, budget, HITL, artefatos e observabilidade.

## Decisão

1. **Um contrato, dois modos.** `opencode` continua usando o daemon via REST/SSE
   (`OpenCodeAdapter`). Os demais harnesses são processos CLI
   (`CliAdapter`) com o workspace como cwd e o prompt injetado por placeholder.
2. **Normalização JSON-lines.** O adapter entende Claude Code `stream-json`,
   Codex `--json`, o protocolo genérico (`text`, `tool_call`, `usage`, `diff`,
   `artifact`, `done`) e texto puro. Tudo vira `RuntimeEvent`.
3. **Execução compartilhada.** `adapters::execution::execute` consome o stream e
   produz o mesmo `RunOutcome` do OpenCode: status, usage, summary, diffs,
   writes, permissões e erro. Settlement, artefatos e notificações não sabem
   qual harness rodou.
4. **Diffs sem endpoint.** Para CLIs, arquivos alterados vêm de
   `git status --porcelain` ao final do run, preservando proveniência.
5. **Registro em SQLite.** `adapter_registry` (migração 0045) guarda comando,
   argumentos, argumentos de health e capacidades. A UI de Configurações →
   Harnesses edita, verifica (`--version`) e define o padrão.
6. **Seleção no DSH.** `tasks.preferred_harness` sobrevive ao dispatch;
   `execution_queue.resolved_harness` congela a escolha. O router decide
   `manual_harness` → padrão do workspace → `opencode`, e expõe `harness` no
   resultado do teste de roteamento.
7. **Falha explícita.** Harness sem binário/comando falha o run com mensagem
   clara (`harness_unavailable`) em vez de exigir o daemon OpenCode.

## Consequências

- Um workspace pode comparar harnesses na mesma tarefa trocando o padrão ou a
  preferência da tarefa; o histórico mantém o harness usado.
- Ferramentas que dependem de permissões interativas do OpenCode não são
  emuladas para CLIs; o processo herda as próprias regras de aprovação.
- O daemon continua obrigatório apenas para `opencode`.
