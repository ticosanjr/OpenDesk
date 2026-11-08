# Pacotes iniciais (Starter Packs)

Pacotes são conjuntos opcionais de agentes, skills, comandos, receitas,
rotinas e pastas. Nada é aplicado automaticamente: o usuário escolhe na página
**Pacotes** ou cria um workspace a partir de um template.

## Garantias

- Instalação mostra prévia, conflitos e requisitos.
- Arquivos idênticos são ignorados (reinstalação idempotente).
- Conflito nunca é sobrescrito sem confirmação; o Studio faz backup.
- A UI permite escolher conflito por arquivo. Os não selecionados são mantidos
  e o pacote aparece como `personalizado`.
- Pacotes instalados mostram `atualização` quando a versão do bundle é maior.
- Remoção apaga apenas arquivos cujo checksum ainda é o instalado.
- Rotinas entram **inativas**.
- Receitas com efeito externo (e-mail, publicação, envio) terminam em HITL.
- Nenhum pacote grava modelo fixo, senha ou token.

## Catálogo

| Categoria | Pacotes |
|---|---|
| Base | `meta-forge`, `essenciais` |
| Pesquisa | `deep-research`, `wiki-karpathy`, `estudos-tutor` |
| Negócios | `vendas-pre-vendas`, `pos-vendas`, `lancamento-produto`, `imobiliario`, `contabilidade-pj` |
| Escritório | `rh-recrutamento`, `administrativo-escritorio`, `notas-fiscais`, `juridico`, `suporte-cliente` |
| Conteúdo | `blog-writer`, `book-writer`, `youtube-scripts`, `noticias-radar`, `titulos-thumbs` |
| Casa | `casa-financas`, `saude-bem-estar`, `viagens` |
| Produtividade | `email-zero`, `reunioes` |

Templates de workspace: `fabrica-capacidades`, `pessoal-casa`, `conteudo`,
`empresa`, `pesquisa-wiki`, `comercial`, `escritorio`.

## Estrutura do bundle

```
starter-packs/
  catalog.json
  packs/<id>/
    pack.json
    README.md
    agents/*.md
    skills/<skill>/SKILL.md
    commands/*.md
    recipes/*.yaml
    routines/*.json
  templates/<id>.json
```

O bundle é embutido no binário com `include_dir`, então funciona offline.

## Manifesto

```json
{
  "schema": 1,
  "id": "meu-pacote",
  "version": "1.0.0",
  "name": "Meu pacote",
  "category": "produtividade",
  "description": "O que ele resolve.",
  "icon": "package",
  "color": "#3b82f6",
  "recommended": false,
  "requires": { "tools": [], "integrations": [] },
  "contents": {
    "agents": ["meu-agente.md"],
    "skills": ["minha-skill"],
    "commands": ["meu-comando.md"],
    "recipes": ["minha-receita.yaml"],
    "routines": ["minha-rotina.json"],
    "folders": ["input", "output"]
  }
}
```

Regras:
- IDs únicos e slugs (`a-z`, `0-9`, `-`).
- Agentes/comandos: um `.md` com frontmatter válido.
- Skills: pasta com `SKILL.md` e arquivos extras opcionais.
- Receitas: formato canônico `flow`, `triggers: []` por padrão.
- Rotinas: JSON de `CreateScheduleInput` com `is_active: false`.
- Caminhos relativos; `..`, absolutos e links simbólicos são rejeitados.
- Rotinas só podem apontar para receitas do mesmo pacote.

## Adicionar um pacote

1. Crie `starter-packs/packs/<id>/`.
2. Adicione `pack.json`, agentes, skills, comandos, receitas e rotinas.
3. Acrescente o ID em `starter-packs/catalog.json`.
4. Rode `cargo test starter_packs`.
5. Instale em um workspace de teste pela API ou pela página Pacotes.

O teste `validate_bundle` valida manifestos, caminhos, receitas (parser real),
rotinas e colisões dentro de cada template. Nenhuma mudança de código é
necessária para um novo pacote.

## API

- `GET /api/starter-packs` — catálogo e status no workspace.
- `GET /api/starter-packs/{id}` — detalhe.
- `POST /api/starter-packs/{id}/preview` — plano de instalação.
- `POST /api/starter-packs/{id}/install` — instala; aceita `overwrite` por
  arquivo e `skip_conflicts` para preservar os não selecionados.
- `DELETE /api/starter-packs/{id}` — remove com preservação.
- `GET /api/starter-packs/templates` — templates de workspace.
- `POST /api/starter-packs/templates/create` — cria workspace e instala packs.

## Segurança

- Contabilidade, finanças, saúde e jurídico incluem aviso de escopo e exigem
  revisão humana.
- E-mail e suporte nunca enviam sem aprovação.
- Rotinas começam inativas e podem ser ignoradas.
- Remover um pacote não apaga arquivos modificados nem rotinas já executadas.
