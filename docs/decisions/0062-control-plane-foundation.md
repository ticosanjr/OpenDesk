# 0062 — Fundação do control plane v1.0

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o OpenDesk 0.59.0 é um desktop acoplado ao OpenCode, sem contrato
  de harness, sem daemon independente e sem os schemas de empresa, memória,
  canais e workers exigidos pelo programa v1.0. O escopo aprovado é single-user,
  self-hosted, P0+P1+P2, sem prazo.

## Decisão

1. **Contrato de harness**: criar `runtime::AgentRuntimeAdapter`,
   `RuntimeCapabilities` e `RuntimeEvent` normalizado (início/fim, texto,
   raciocínio, ferramentas, permissões, usage, diff, artefato e erro). OpenCode
   V2 será o primeiro adapter; Claude Code, Codex, Pi e Hermes entram na mesma
   interface.
2. **Registry de adapters** com id, capabilities, obtenção e listagem.
3. **Binário `opendeskd`** como entrada headless separada da UI, reutilizando o
   estado/API atuais nesta fase. A extração física de `opendesk-core` e o
   empacotamento como serviço ficam para a onda 1.
4. **Schema foundational** (migração 0042) para:
   - handoffs;
   - tickets;
   - rotas de canal e trust level;
   - namespaces, ACL, notas e entidades de memória;
   - workers;
   - registry de adapters;
   - eventos normalizados de runtime.
5. **Registro de variáveis de prompt**: variáveis atuais e futuras
   (`memory.core`, `memory.context`, `handoff.contract`,
   `channel.trust_level`, `harness`) expostas pela API de templates.
6. **Daemon com SIGTERM/SIGINT** e desligamento gracioso do cron, I/O bus, fila
   e daemon OpenCode.

## Consequências

- As ondas seguintes podem adicionar adapters, canais, memória e MCP sem outra
  reescrita de schema.
- O `opendeskd` ainda compartilha o crate com a UI nesta etapa; a separação
  física é o primeiro item da onda 1.
- Nenhum release é feito entre ondas; o próximo release só ocorre depois da
  onda 9.
