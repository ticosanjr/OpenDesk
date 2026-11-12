# Proposta de interface — instrumento de precisão

Revisão feita em 2026-11-12 sobre a interface desktop (1440×900 e 1100×700,
com a janela real em 1920×1008). O objetivo não é trocar a identidade do
OpenDesk, e sim remover atrito operacional: menos ruído, hierarquia clara e
decisões visíveis no momento em que importam.

## Direção

**Um control plane silencioso.** A interface deve parecer um instrumento de
precisão: superfícies escuras, um único acento vindo do workspace, tipografia
com papéis claros e dados densos separados por espaço — não por caixas dentro
de caixas.

Princípios:

1. **Um trabalho por zona.** Título, ação primária, conteúdo e metadados não
   competem. Filtros e status secundários saem da linha principal.
2. **O acento marca estrutura.** A cor do workspace aparece em foco, seleção,
   navegação ativa e divisores. Azul, verde, âmbar e vermelho continuam
   reservados a informação, sucesso, atenção e falha.
3. **Densidade legível.** Corpo em 13,5px, metadados em 12px, overline em 11px;
   mono apenas para números, IDs, caminhos e código.
4. **Divisores antes de cards.** Um card dentro de outro card é ruído. Use
   divisórias, superfícies internas e listas.
5. **Revelação progressiva.** O caminho principal fica visível; filtros
   avançados, histórico e explicadores aparecem sob demanda.
6. **Toda ação tem estado.** Hover, foco, carregando, vazio, erro, sucesso e
   desabilitado são parte do componente, não remendo de página.

## O que já foi corrigido

- Shell: topbar sem caminho duplicado, fila legível (“0 ativos · 0 na fila”),
  busca sempre disponível, orçamento real vindo da API, ação de novo workspace
  com nome acessível e confirmação antes de reiniciar o engine com jobs ativos.
- Navegação: sidebar com ícones distintos, separadores no modo recolhido,
  preferência persistida, compactação em 700px e recolhimento automático abaixo
  de 1200px.
- Tipografia: títulos de página em 24px, corpo em 13,5px, overlines decorativas
  removidas; DESIGN.md e ADR 0046 registram a escala.
- Filtros: `FilterBar`, `FilterSearch` e `FilterSelect` compartilhados em
  Execuções, Artefatos, Arquivos e Kanban, com limpeza por botão e Esc.
- Observabilidade: série diária contínua (barras voltaram a renderizar), eixo
  sem quebra de linha, rótulos de saúde em português e plural correto.
- Modais: tamanhos `sm/md/lg/xl`, confirmações compactas, foco inicial seguro em
  ações destrutivas e descrição acessível.
- Paleta de comandos: ação explícita de busca completa no workspace, separando
  navegação rápida de busca profunda.
- Acessibilidade: foco visível de fallback, nomes acessíveis em controles
  icônicos, switch compartilhado no TLS e estados vazios compactos em cards.

## Próximas fases recomendadas

### 1. Kanban com views salvas (P1) — parcialmente feito na onda 8

Agente e prioridade foram para o popover **Filtros**, com chips ativos e o
preset **Somente atenção**; as setas de rolagem cobrem telas de 1100px. Ainda
falta salvar combinações nomeadas (Minhas tarefas / Automações / Revisão) e
transformar a concorrência em um ajuste da fila.

### 2. Configurações com IA mais curta (P1)

Dez abas em uma linha funcionam em 1440px, mas a leitura ainda é de catálogo.
Agrupar em quatro blocos — **Geral**, **Motor e execução**, **Dados e
segurança**, **Sobre** — com uma navegação secundária vertical ou um select
compacto. O conteúdo ganha largura e a pessoa não precisa varrer dez rótulos.

### 3. Governança como organograma real (P1) — feito na onda 7

A tabela compacta (Agente, Escopo, Execuções 30d, Sucesso, Budget, Ações)
substituiu os cards aninhados; a hierarquia aparece por indentação e o
organograma fica reservado às relações de delegação.

### 4. Dashboard com uma faixa de status (P2) — feito na onda 7

Os quatro KPIs viraram uma faixa única com divisores, reduzindo altura e
antecipando o conteúdo operacional. O Attention Card continua acima de tudo
quando existe uma decisão pendente.

### 5. Busca única (P2)

A paleta navega e agora oferece a busca completa; a página `/search` ainda é
outro lugar. Unificar o resultado em uma lista com filtros por tipo e manter a
paleta como atalho torna a busca previsível. Medir o tempo até o primeiro
resultado útil.

### 6. Consistência de dados (P2)

Padronizar números tabulares, alinhamento à direita em tabelas, rótulos de
status traduzidos (`RUNNING` → “executando”) e a mesma fonte para gasto do dia
no topbar, dashboard e Budget Guard. Cores de status nunca devem carregar
significado sozinhas.

## Critérios de aceite da próxima onda

- Nenhum overflow horizontal em 1024×768, 1100×700, 1440×900 e 1920×1080.
- Toda rota alcançável com a sidebar recolhida; foco visível em todo controle.
- Confirmar ação destrutiva com foco no caminho seguro.
- Nenhum texto de interface abaixo de 11px; corpo em 13,5px.
- `npm run check:changed` verde; detector Impeccable sem achados; axe sem
  violações críticas ou sérias nas rotas principais.

## Verificação desta revisão

- `npm run check:changed` — ESLint, TypeScript e 158 testes Vitest.
- Detector Impeccable em `src` — sem achados.
- Inspeção visual em 1440×900 e 1100×700, incluindo dashboard, Kanban,
  Execuções, Observabilidade, Configurações, Integrações, Roteador, Calendário,
  Artefatos, Arquivos e a paleta de comandos.
