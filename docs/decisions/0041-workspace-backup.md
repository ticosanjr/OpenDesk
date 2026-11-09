# ADR 0041 — Backup e restauração de workspace

## Contexto

O SQLite já tinha backup automático antes de migrações, mas os arquivos do
workspace — entradas, saídas, `.opencode/`, agentes, skills e configuração —
não tinham uma cópia portátil. Isso impedia recuperação completa e migração
entre máquinas.

## Decisão

- Cada workspace pode gerar um `.zip` completo em
  `backups/workspaces/<workspace_id>/`.
- O arquivo inclui um manifesto `opendesk-backup.json`, contagem de arquivos,
  bytes e versão do formato.
- São excluídos diretórios de dependência/cache: `.git`, `node_modules`,
  `target`, `dist`, `build`, `.next`, `.turbo`, `.venv`, `venv`, `__pycache__`,
  `.cache`.
- Symlinks são ignorados no backup e recusados na extração. Caminhos absolutos
  e `..` são rejeitados; há limite de 2 GiB e 100 mil arquivos na restauração.
- O ZIP é verificado por SHA-256 antes de restaurar.
- A restauração cria **um novo workspace** a partir do backup. Ela nunca
  sobrescreve o workspace atual e exige destino vazio ou inexistente.
- O workspace novo é registrado no SQLite normalmente, com nome e descrição de
  proveniência.
- A UI fica em **Configurações → Backups**; o CLI expõe
  `opendesk backups list/create/restore/download/delete`.

## Consequências

- Recuperação completa sem depender do banco ou da máquina original.
- Restauração segura por padrão, sem sobrescrita destrutiva.
- Arquivos grandes de dependência não inflam o backup; saídas e configuração
  nativa continuam incluídas.
- O backup não substitui o backup do SQLite: são camadas complementares.
