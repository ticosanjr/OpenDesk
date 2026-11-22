# 0051 — Deduplicação persistente de Folder Watch

- **Status:** aceito
- **Data:** 2026-11-14
- **Contexto:** o Folder Watch usava um `HashSet` em memória. Ao reiniciar o
  OpenDesk o conjunto era perdido, `process_existing` voltava a processar todos
  os arquivos e uma execução falha removia o arquivo do conjunto para tentar de
  novo mesmo sem alteração. O mesmo arquivo podia gerar várias execuções.

## Decisão

1. Persistir o estado por `(recipe_id, trigger_path, file_path)` em
   `recipe_file_state`.
2. O fingerprint é `mtime` em nanossegundos + tamanho. O arquivo só é
   processado quando o fingerprint muda ou quando não há estado registrado.
3. `process_existing=false` grava `SEEN` para os arquivos existentes sem
   executá-los.
4. Falha ao **iniciar** uma execução ainda pode ser tentada de novo dentro de
   `retry_minutes`; uma execução que terminou `FAILED`, `REJECTED` ou
   `TIMED_OUT` mantém o fingerprint e não é reprocessada até o arquivo mudar.
5. No primeiro scan após a atualização, arquivos já presentes em `recipe_runs`
   são conciliados com o estado atual (mtime <= início do run vira `SEEN`), para
   não gerar uma última reprocessação em massa.

## Consequências

- Reinícios, recompilações e falhas não duplicam execuções do mesmo arquivo.
- Editar ou substituir o arquivo (novo mtime/tamanho) volta a disparar a
  receita normalmente.
- Um arquivo que falhou e não muda fica registrado como `FAILED`; uma futura
  ação manual de reprocessamento pode ser adicionada sem alterar o scanner.
- Arquivos cujo conteúdo muda preservando exatamente mtime e tamanho não são
  detectados por esse fingerprint; é um caso raro em cópias/edições normais.
