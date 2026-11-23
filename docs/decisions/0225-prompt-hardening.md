# 0225 — Prompts internos blindados e com orçamento

- **Status:** aceito
- **Data:** 2026-11-23
- **Contexto:** o prompt interno era uma concatenação plana de seções, sem
  fronteira estrutural para dados de canal, sem teto de tokens e sem adaptação
  por harness. Modelos menores e entradas externas ficavam expostos a
  instruções injetadas e a contexto ilimitado.

## Decisão

1. `context_packer` separa o prompt em blocos XML: `<system_rules>`, `<memory>`,
   `<task>`, `<output_contract>` e `<untrusted_data>`.
2. Dados de canal, follow-up e chat entram em `<untrusted_data>` com escape de
   `&`, `<` e `>`; a diretiva fixa declara que o conteúdo é passivo e não pode
   alterar escopo nem executar ações destrutivas.
3. `PromptBudget` aplica prioridades `P0` a `P5`:
   - `P0` segurança/regras e `P1` tarefa/handoff nunca são cortados;
   - `P2` memória do usuário, `P3` memória estruturada, `P4` `MEMORY.md` e `P5`
     histórico são truncados com `[TRUNCATED: N tokens omitidos]`.
4. O orçamento padrão é 16 000 tokens de entrada, com 2 048 reservados para
   saída; modelos desconhecidos usam esse fallback sem panic.
5. `harness_id` no `PackContext` remove instruções exclusivas do OpenCode
   (como `OpenCode Browse`) quando o run usa outro harness.
6. `output_schema`, quando presente, vira `<output_contract format="json_strict">`
   com o JSON minificado.
7. `prompt_preview` e o preview do chat usam o mesmo `render_sections_with`;
   os starter packs prioritários deixam de ser stubs e um teste falha se o
   boilerplate genérico voltar.

## Consequências

- O prompt deixa de ser uma string opaca; o preview mostra a mesma estrutura
  enviada ao modelo.
- Conteúdo externo não consegue fechar o bloco `untrusted_data`.
- Seções de baixa prioridade podem ser cortadas de forma determinística, mas a
  tarefa e as regras de segurança permanecem.
- `recipe_call`/steps com `output_schema` passam a ter contrato explícito.
- Shell: **358 testes**; engine: **139**; Vitest: **267**.
