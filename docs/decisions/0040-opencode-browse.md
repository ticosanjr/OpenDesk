# ADR 0040 — Integração com OpenCode Browse

## Contexto

Tarefas de pesquisa, comparação de preços, leitura de sites e preenchimento de
formulários precisam de um navegador real, com logins e 2FA do usuário. O
OpenCode Browse oferece isso como MCP (`browse`) + extensão Chromium, com
handoff humano e abas isoladas.

## Decisão

- O OpenDesk **não** implementa automação de navegador. Ele configura e orienta
  o MCP `browse`, e o OpenCode executa as ferramentas.
- Tarefas podem ser marcadas com a tag `needs-browser` pela UI (“Precisa
  navegar na internet”) ou por detecção conservadora de URL/palavras-chave.
- No dispatch de uma tarefa web, o OpenDesk garante:
  - entrada MCP `browse` no `opencode.jsonc` do workspace (binário `browse` se
    existir, senão `npx -y @opencode/browse mcp`);
  - skill nativa `browse-web` com regras de uso, allowlist e handoff;
  - seção de navegação injetada no prompt da tarefa.
- A ativação é idempotente: só grava config/skill quando faltam.
- A UI de **Integrações → Browser** mostra binário, MCP, skill, comando e
  permite ativar/desativar por escopo.
- Login, 2FA, pagamento e captcha são sempre handoff para o usuário; o agente
  não tenta contornar. O OpenDesk não armazena credenciais do Browse.
- O catálogo de MCPs e o catálogo externo incluem o Browse como fonte curada.

## Consequências

- Tarefas web funcionam sem duplicar runtime de browser ou CDP no OpenDesk.
- Sem a extensão instalada, o MCP existe mas o agente reporta a falha; a UI
  aponta para a instalação.
- A detecção por URL/palavra-chave pode ativar o Browse em tarefas que só citam
  um link; a skill orienta o agente a usá-lo apenas quando necessário.
- Configuração e skill são reversíveis; desativar remove só o MCP.
