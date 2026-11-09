# ADR 0035 — Receitas com chave composta por workspace

## Contexto

Packs instalam as mesmas receitas em vários workspaces. O índice
`recipes_index` usava `id` como chave primária global, então o mesmo id em dois
workspaces colidia: o `ON CONFLICT` movia a linha de um workspace para o outro e
o resultado dependia da ordem dos syncs (startup, watcher, instalação). Isso
fazia receitas sumirem da UI e podia apontar runs para o arquivo errado.

## Decisão

- `recipes_index` passa a ter chave primária `(workspace_id, id)`.
- `recipe_runs` referencia `(workspace_id, recipe_id)` e mantém seu próprio id
  global.
- `schedules.recipe_id` deixa de ter FK de coluna única; o vínculo é validado
  pelo workspace da rotina e pelo engine no momento da execução.
- Toda leitura de receita (`get`, `yaml_for`, execução, triggers) exige o
  `workspace_id`, eliminando consultas ambíguas.
- A reconciliação de diretório é serializada por workspace para que watcher e
  instalação de pack não apaguem linhas um do outro.

## Consequências

- O mesmo pacote pode ser instalado em qualquer número de workspaces sem
  colisão.
- IDs de receita continuam amigáveis e iguais ao `id` do YAML dentro de cada
  workspace.
- Chamadas internas passam a carregar o workspace explicitamente; uma consulta
  global por id deixa de ser possível por design.
- A migração reconstrói as tabelas de receitas, runs e HITL, além de `schedules`,
  com backup automático do SQLite antes da execução.
