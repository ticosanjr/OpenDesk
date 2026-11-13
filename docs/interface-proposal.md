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

### 1. Kanban com views salvas (P1) — feito nas ondas 8, 11 e 12

Agente e prioridade foram para o popover **Filtros**, com chips ativos, preset
**Somente atenção**, views nomeadas persistidas e setas de rolagem para
1100px. Cards com HITL mostram o motivo e respondem por um modal focado. Falta
apenas transformar a concorrência em um ajuste explícito da fila.

### 2. Configurações com IA mais curta (P1) — parcialmente feito na onda 10

As abas agora têm grupos visuais (Geral, Motor, Operação, Workspace, Sobre) e
Integrações segue o mesmo padrão (Mensageria, I/O, Sistema). Ainda falta
reduzir para quatro blocos com navegação secundária/vertical, mas a varredura
de dez rótulos ficou mais curta.

### 3. Governança como organograma real (P1) — feito na onda 7

A tabela compacta (Agente, Escopo, Execuções 30d, Sucesso, Budget, Ações)
substituiu os cards aninhados; a hierarquia aparece por indentação e o
organograma fica reservado às relações de delegação.

### 4. Dashboard com uma faixa de status (P2) — feito na onda 7

Os quatro KPIs viraram uma faixa única com divisores, reduzindo altura e
antecipando o conteúdo operacional. O Attention Card continua acima de tudo
quando existe uma decisão pendente.

### 5. Busca única (P2) — feita na onda 13

A paleta agora consulta `/api/search` com o mesmo ranking da página e agrupa
os resultados por tipo, com atalho para a busca completa. Buscas recentes são
persistidas e compartilhadas entre a paleta e `/search`, com remoção por item.

### 6. Consistência de dados (P2)

Padronizar números tabulares, alinhamento à direita em tabelas, rótulos de
status traduzidos (`RUNNING` → “executando”) e a mesma fonte para gasto do dia
no topbar, dashboard e Budget Guard. Cores de status nunca devem carregar
significado sozinhas.

### 7. Produto operacional (P2) — feito na onda 13

- Dashboard: Attention Card ganhou resumo por categoria e o modo compacto com
  “ver detalhes”; atividades recentes agrupam por dia e filtram runs/artefatos.
- Execuções: tabela opcional com status, agente, duração, tokens e custo, com
  ordenação; o run detalhado filtra eventos por ferramenta/erro/conversa e
  expande ou recolhe todas as seções.
- Viewers: ZIP lista entradas sem extrair e oferece download autenticado;
  artefato fora do workspace explica a política e mantém abrir/copiar caminho.
- Pacotes: “Atualizar todos” para pacotes com atualização, gerenciamento em
  lote com remoção confirmada e estado vazio próprio para favoritos. O
  Impeccable segue externo por design e isso fica explícito na tela.

## Critérios de aceite da próxima onda

- Nenhum overflow horizontal em 1024×768, 1100×700, 1440×900 e 1920×1080.
- Toda rota alcançável com a sidebar recolhida; foco visível em todo controle.
- Confirmar ação destrutiva com foco no caminho seguro.
- Nenhum texto de interface abaixo de 11px; corpo em 13,5px.
- `npm run check:changed` verde; detector Impeccable sem achados; axe sem
  violações críticas ou sérias nas rotas principais.

## Verificação desta revisão

- `npm run check:changed` — ESLint, TypeScript e Vitest (196 testes).
- Detector Impeccable em `src` — sem achados.
- Inspeção visual em 1440×900 e 1100×700, incluindo dashboard, Kanban,
  Execuções, Observabilidade, Configurações, Integrações, Roteador, Calendário,
  Artefatos, Arquivos e a paleta de comandos.
- Goldens visuais de **18 rotas + 5 modais** em 700/1024/1100/1440/1920
  (`npm run test:visual`), além do gate completo com 93 testes Playwright.
- Axe em 1024/1440/1920 e suíte funcional 74/74; Rust em 377 testes, clippy e
  cobertura 64,2% total/70,8% core.
