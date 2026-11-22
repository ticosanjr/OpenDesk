# 0105 — Sync delta do workspace para nós

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** um worker remoto precisava do workspace, mas não havia um
  contrato de manifesto/delta: era necessário copiar tudo manualmente ou montar
  um volume compartilhado.

## Decisão

1. `workspace_sync::manifest` percorre o workspace e gera entradas ordenadas
   (`path`, `size`, `modified_ms`, `sha256`) ignorando `.git`, `node_modules`,
   `target`, `output` e `.cache`. Arquivos acima de 8 MB usam tamanho/mtime em
   vez de hash.
2. `delta(base, target)` classifica adicionados, alterados e removidos e calcula
   os bytes necessários.
3. `bundle` cria um `tar.zst` apenas com arquivos adicionados/alterados. Remoções
   são reportadas, mas não apagam nada automaticamente.
4. `apply` valida cada caminho (sem absoluto, `..` ou componentes especiais),
   aceita apenas arquivos regulares e suporta `dry_run`.
5. API:
   - `GET /api/workspaces/{id}/sync/manifest`
   - `POST /api/workspaces/{id}/sync/bundle` com `base` (manifesto do nó)
   - `POST /api/workspaces/{id}/sync/apply` com `bundle_path` e `dry_run`.
6. Bundles só podem ser lidos de `<data_dir>/sync`; o caminho é canonicalizado
   antes da extração.
7. O manifesto e o bundle são operacionais; não há mTLS nem autenticação de nó
   neste contrato ainda.

## Consequências

- Um nó remoto baixa só o delta e aplica localmente, sem mount compartilhado.
- Remoções são explícitas no relatório para decisão humana.
- O workspace sincronizado inclui os arquivos do usuário (inclusive `.env`);
  proteger o canal/transportar apenas para nós confiáveis é responsabilidade da
  operação.
- mTLS, rotação de certificados e sync automático no `opendesk node run`
  continuam pendentes em REQ-10.
