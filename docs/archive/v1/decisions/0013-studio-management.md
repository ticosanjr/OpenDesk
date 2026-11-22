# ADR 0013 — Gestão completa do OpenCode Studio

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** o Studio já fazia CRUD básico, mas não mostrava o efeito
  combinado de global + workspace, não ativava/desativava skills e comandos, não
  copiava/movia entre workspaces e não instalava itens de fontes comuns.

## Decisão

- O Studio trabalha com quatro tipos (`agent`, `skill`, `command`, `mcp`) e dois
  escopos persistentes (`global`, `workspace`). A visão **Efetivo** combina os
  dois e marca overrides.
- Operações genéricas:
  - `duplicate`, `copy`, `move` entre qualquer workspace e global;
  - `toggle` de ativação;
  - `backups` e `restore` por item;
  - `install/preview` e `install/apply`.
- Ativação:
  - agentes e MCPs usam `disabled` nativo;
  - skills e comandos são movidos para `.opencode/disabled/{skills,commands}`,
    que o OpenCode não carrega, e voltam ao ativar.
- Instalação:
  - GitHub, URL, pasta/arquivo local, zip, JSON de MCP e npm;
  - catálogo curado de MCPs;
  - preview com seleção item a item, destino e conflito;
  - nenhum script é executado;
  - limites de tamanho e proteção contra path traversal/zip-slip.
- Backups gravam `metadata.json` com o caminho original; restaurar exige que o
  backup pertença ao arquivo editado.
- O projeto é limpo após cada release (`scripts/clean.sh`), removendo `target`,
  `dist`, `coverage`, `test-results` e temporários, mantendo apenas as 3 últimas
  releases.

## Consequências

- Copiar/mover é sempre explícito e nunca sobrescreve sem confirmação.
- O Studio mostra itens desativados e permite reativá-los.
- A instalação não depende de executar código de terceiros.
- O diretório do projeto deixa de acumular dezenas de GB de build.
