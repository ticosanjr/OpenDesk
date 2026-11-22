# Regressão visual

Os goldens vivem em `tests/visual/surfaces.spec.ts-snapshots/` e cobrem todas as
rotas e os modais principais.

## Cobertura

- **18 rotas**: Dashboard, Busca, Kanban, Calendário, Execuções, Chat, Rotinas,
  Receitas, Artefatos, Arquivos, Pacotes, Studio, Governança, Observabilidade,
  Aprendizado, Roteador, Integrações e Configurações.
- **5 modais**: nova tarefa, detalhes da tarefa, nova rotina, instalação de
  pacote e command palette.
- **Larguras**: 1440 em todas; 1024/1920 nas superfícies densas; 700 e 1100 em
  Dashboard, Kanban, Pacotes e Configurações.
- 49 imagens, ~6 MB.

Comandos:

```bash
npm run test:visual          # compara com os goldens
npm run test:visual:update   # regrava após mudança visual intencional
OPENDESK_SKIP_VISUAL=1 scripts/gate.sh   # pula os goldens no gate completo
```

## Determinismo

- O seed cria workspace, tarefas, run e artefato antes das capturas.
- O Calendário usa `?date=2024-01-15`, uma data fixa no passado, para não
  depender do mês corrente nem do destaque de “hoje”.
- O modal de rotina instala um relógio fixo em `2026-11-13T12:00:00Z`, porque
  a prévia lista os próximos disparos a partir de “agora”.
- O viewport usa `timezoneId: "America/Sao_Paulo"` para o fuso não variar entre
  máquinas.
- `maxDiffPixelRatio` é 1,5% para absorver rótulos relativos (“há menos de um
  minuto”) e pequenos números do dia.
- Os sufixos `-linux` vêm do sistema que gerou o golden; rodar em outro SO gera
  um conjunto próprio.

## Quando atualizar

Atualize os goldens somente quando a mudança visual for intencional, e revise
as imagens antes de commitar:

1. Rode `npm run test:visual` e confirme que a diferença é esperada.
2. Rode `npm run test:visual:update`.
3. Inspecione pelo menos um golden por área alterada.
4. Rode `npm run test:visual` de novo.
5. Registre a mudança em `docs/archive/v1/progress.md` (e em `DESIGN.md` se for regra).

## Limites

- Dados do servidor usam a data real da execução: o KPI “gasto hoje” e séries
  diárias de Observabilidade podem exigir regravação quando o dia muda.
- Não há CI; os goldens rodam no gate completo local (`scripts/gate.sh`) e
  podem ser pulados com `OPENDESK_SKIP_VISUAL=1`.
- Axe continua sendo a verificação de acessibilidade; os goldens cobrem layout.
