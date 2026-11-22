# 0056 — Tela única de execução

- **Status:** aceito
- **Data:** 2026-11-14
- **Contexto:** para rodar com outro agente ou modelo era preciso editar a
  tarefa, salvar e só então executar. O cancelamento ficava no controlador da
  fila, e comandos/skills só podiam ser adicionados editando a descrição. O
  usuário precisava alternar entre Kanban, fila e execução.

## Decisão

1. **Um modal de execução** aberto pelos botões Executar do Kanban, da lista e
   do card de detalhes. Ele é a tela única para configurar, iniciar, acompanhar
   e parar um run.
2. **Configuração por execução**: modo, agente, modelo e effort vão no corpo do
   `POST /api/tasks/{id}/dispatch`/`approve-plan`, sem alterar a tarefa. Campo
   vazio continua significando roteador automático.
3. **Comando/skill/instrução avulsa**: o mesmo `SlashTextarea` do chat e do
   formulário oferece comandos e skills; o texto é enviado em
   `prompt_append`, entra como “Instruções desta execução” e não é persistido.
   Limite de 8.000 caracteres.
4. **Stop no mesmo lugar**: com o run ativo, o modal mostra o estado ao vivo e
   troca o botão por “Parar execução”, que usa a transição `CANCELLED` e
   cancela o job ativo. A tarefa pode ser reaberta depois.
5. **Acompanhamento**: o modal atualiza a tarefa por polling curto e por
   eventos de tarefa/run, mostra fase, último run e diretório de saída, e liga
   para a execução completa. Se o plano estiver aguardando aprovação, o botão
   vira “Aprovar plano e executar”.

## Consequências

- Trocar de agente/modelo para um teste deixa de exigir edição permanente.
- Comandos e skills podem ser usados por execução, sem sujar a descrição.
- Iniciar e parar ficam no mesmo lugar, reduzindo idas ao controlador da fila.
- O prompt de uma execução pode ser reproduzido no Preview de prompt; o
  `prompt_append` é efêmero e não aparece no preview salvo.
