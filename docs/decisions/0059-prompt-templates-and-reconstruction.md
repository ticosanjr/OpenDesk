# 0059 — Templates de prompt e visão reconstruída do prompt final

- **Status:** aceito
- **Data:** 2026-11-15
- **Contexto:** o prompt era montado por strings fixas em `context_packer`, o
  preview não refletia o texto final e o OpenCode carregava agente, skills,
  AGENTS.md e comandos sem que o usuário pudesse inspecionar ou configurar isso.

## Decisão

1. **Seções estruturadas.** `context_packer::pack_sections` devolve uma lista de
   seções com `id`, `label`, `source` e conteúdo. `pack` é o join dessas seções;
   o preview usa exatamente a mesma lista, eliminando a divergência entre o que
   aparece e o que é enviado.
2. **Templates editáveis.** Os blocos permanentes, critérios de conclusão,
   contexto pessoal, memória, metadados/descrição da tarefa, diretório de saída,
   arquivos de entrada, fases PLAN/BUILD/AUDIT, navegação web, níveis de cuidado
   e contexto OpenDesk/HITL vivem em `PromptTemplates`.
   - Placeholders (`{{task_title}}`, `{{output_directory}}`, `{{plan_block}}`,
     etc.) são substituídos na renderização; placeholders desconhecidos ficam
     visíveis no preview.
   - Workspace tem precedência sobre o padrão global; chaves ficam em
     `app_settings` (`prompt_templates` e `prompt_templates:<workspace>`).
   - `GET/PUT /api/prompt/templates` e `POST /api/prompt/templates/reset`.
3. **Camada OpenCode.** O preview lê e mostra prompt do agente, `AGENTS.md`
   global/workspace, skills (incluindo corpo), comandos e servidores MCP. A aba
   “Final (reconstruído)” concatena o prompt exato do OpenDesk com essa camada.
4. **Limite explícito.** O OpenCode monta o system prompt interno e os schemas
   de ferramentas no payload do provedor; isso não é exposto pela API. A visão
   final é uma reconstrução de tudo o que o OpenDesk envia e o OpenCode lê dos
   arquivos nativos, com esse aviso na UI.

## Consequências

- Qualquer texto automático que o OpenDesk injeta pode ser auditado e alterado.
- O preview deixa de usar heurística de títulos e passa a refletir a montagem
  real.
- Para o payload byte-a-byte do provedor seria necessário um proxy local ou
  mudança no OpenCode; fica fora deste escopo.
