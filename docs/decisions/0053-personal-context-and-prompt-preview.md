# 0053 — Personal Context e inspeção de prompt

- **Status:** aceito
- **Data:** 2026-11-14
- **Contexto:** o usuário repetia instruções estáveis em várias tarefas e não
  tinha como saber o que exatamente o OpenDesk enviava ao modelo. O prompt era
  montado em `dsh_router::pack`, com MEMORY.md e regras, mas sem proveniência.

## Decisão

1. Adicionar grupos de **Personal Context** com escopo `workspace` ou `global`,
   nome, instruções, ativação e posição. Grupos de workspace vêm antes dos
   globais no prompt.
2. Injetar os grupos ativos automaticamente em:
   - prompts de tarefas e continuações;
   - prompts de rotinas/receitas (via tarefa criada pelo engine);
   - mensagens de chat e Telegram.
3. Limitar cada grupo a 4.000 caracteres e o bloco total a 8.000, para não
   consumir o contexto silenciosamente.
4. Expor `GET /api/tasks/{id}/prompt-preview` com:
   - prompt final exato;
   - seções com origem (permanente, pessoal, memória, tarefa, plano, output,
     inputs, browse, cuidado, HITL);
   - agente/modelo/fase/output;
   - agente e skills que o OpenCode carrega separadamente.
5. Adicionar a aba **Configurações → Prompt** para escolher uma tarefa e
   inspecionar/copiar o prompt e suas seções.
6. A UI de Personal Context fica em Aprendizado, com CRUD, ativação e remoção.

## Consequências

- Instruções estáveis deixam de ser repetidas em cada tarefa.
- O usuário consegue auditar o que entrou no modelo e de qual fonte.
- O preview distingue o que o OpenDesk envia do que o OpenCode carrega por conta
  própria (prompt do agente e skills).
- Contexto pessoal muito grande é truncado de forma determinística.
