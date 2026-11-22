# 0061 — Três abas de prompt e memória com destinos explícitos

- **Status:** aceito
- **Data:** 2026-11-15
- **Contexto:** a aba Prompt acumulava editor e dois previews na mesma tela, os
  nomes/ordem das abas não seguiam o vocabulário do produto e ainda havia
  prompts hardcoded (correção de receita, teste de modelo). O MCP não tinha como
  salvar um aprendizado e não distinguia aprendizado de contexto pessoal.

## Decisão

1. **Configurações → Prompt com três abas**:
   - `Templates de prompts`
   - `Prompts enviados` (tarefas)
   - `Prompts de chat` (Desktop/Telegram/Signal)
2. **Previews de tarefa e chat com quatro abas fixas, nesta ordem**:
   - `Seções` (componentes do prompt)
   - `OpenDesk` (prompt exato montado pelo OpenDesk)
   - `OpenCode` (agente, AGENTS.md, skills, comandos e MCP)
   - `Final reconstruído` (OpenDesk + camada OpenCode)
3. **Cobertura de templates**: `chat_channel` é o prompt do chat; foram
   adicionados `memory_policy`, `recipe_feedback` e `model_test`. Conteúdo
   específico de receita/rotina continua no editor do próprio item; a camada
   OpenCode permanece somente leitura.
4. **Memória via MCP**:
   - `learning_save` salva em `learning_lessons`; `remember=true` também grava
     no `MEMORY.md`. É o destino padrão para “memorize”.
   - `personal_context_save` só é usado com pedido explícito do usuário e exige
     `user_request` com a citação literal. A regra é gravada com
     `source=agent` e `source_quote`.
   - O template `memory_policy` reforça a regra em tarefas e chat.
5. **Migração 0041** adiciona `source` e `source_quote` ao contexto pessoal; a
   UI mostra “via agente” e a citação.

## Consequências

- O usuário encontra editor e previews sem mistura de responsabilidades.
- “Memorize” não polui o contexto pessoal; contexto pessoal tem trilha de
  auditoria.
- Novos prompts automáticos precisam entrar em `PromptTemplates` e nos
  `FIELDS`, cobertos por teste.
