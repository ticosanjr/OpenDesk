# 0068 — Memória estruturada, ACL e avaliação de skills (onda 5)

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o OpenDesk já tinha `MEMORY.md`, contexto pessoal e lições
  aprendidas. Faltava memória consultável por agente, com validade e permissão,
  além de uma leitura objetiva do uso das skills nativas.

## Decisão

1. **Notas estruturadas.** `memory_notes` guarda título, corpo, namespace,
   tipo (`semantic`/`episodic`/`procedural`/`preference`), tags, confiança,
   `trust_level` (`observed`/`verified`/`inferred`), validade e origem.
2. **Namespaces e ACL.** `memory_namespaces` agrupa notas; `memory_acl` define
   `can_read`/`can_write` por agente. Namespace sem nenhuma linha é aberto;
   com linhas, só quem foi listado lê, e só com `can_write=1` escreve.
3. **Busca e injeção.** `render_context` pesquisa título/corpo/tags e completa
   com as notas mais confiáveis, sempre filtrando pela ACL do agente. O bloco
   entra no prompt como `{{structured_memory}}` (editável em Configurações →
   Prompt). `MEMORY.md` continua separado e humano.
4. **Validade.** Notas expiradas somem da busca e são removidas na manutenção
   de seis horas.
5. **MCP.** `memory_save` e `memory_search` expõem a memória aos agentes;
   `OPENDESK_AGENT` (ou o argumento `agent`) define a identidade usada na ACL.
6. **Avaliação de skills.** `/api/skills/eval` lê `.opencode/skills/*/SKILL.md`
   e conta menções nos logs recentes, mostrando sucesso, falha, último uso e
   skills sem uso. Não altera arquivos nativos.

## Consequências

- A memória deixa de ser um arquivo monolítico: dá para separar contexto
  financeiro, de engenharia ou de um cliente com permissões distintas.
- Skills sem uso e com falha recorrente ficam visíveis para limpeza ou revisão.
- A injeção continua barata: no máximo 8 notas e 8 KB por prompt.
