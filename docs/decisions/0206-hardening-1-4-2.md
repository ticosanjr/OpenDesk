# 0206 — Hardening verificável: limites, cobertura por workspace e processos

- **Status:** aceito
- **Data:** 2026-11-22
- **Contexto:** o gate media cobertura apenas no pacote raiz, aceitava arquivos
  de frontend com mais de 1.000 linhas e não impedia `unwrap/expect` novos no
  shell. Spawns de processos longos podiam sobreviver a timeout e a fila/cron
  giravam em polling ocioso.

## Decisão

1. `check-rust-limits.py` ganhou `--no-line-limit` para aplicar só a regra de
   `unwrap/expect` em `src-tauri/src`, com baseline própria e vazia.
2. `check-frontend-limits.py` limita arquivos `.ts/.tsx` de produção a 500
   linhas (testes e `.d.ts` ficam fora) e roda no gate e no `check:changed`.
3. O gate usa `cargo llvm-cov --workspace`; o piso sobe para 64,0% total e
   80,5% no escopo de negócio (core, domain, storage, engine e recipes).
4. Todo spawn longo usa `kill_on_drop(true)`; em Unix também `process_group(0)`.
   `terminate_child` envia SIGTERM ao grupo, espera 3–5 s e escala para SIGKILL;
   `terminate_pid` faz o mesmo para o daemon desanexado.
5. Fila e cron usam `Notify`; a fila dorme até o lease mais próximo e o cron
   até a próxima edição. `sleep` de 30 s/60 s fica apenas como rede de
   segurança para mudanças de relógio. O shutdown drena os jobs por até 5 s.
6. Componentes de frontend acima de 500 linhas são divididos por seção, sem
   alterar DOM, classes, texto ou semântica de estado. Polling só permanece
   como fallback quando já existe evento WebSocket.

## Consequências

- Regressões de tamanho, `unwrap` e cobertura passam a falhar no gate.
- O número de cobertura fica comparável entre releases porque inclui todos os
  crates do workspace.
- A meta de 75% exige testes novos; o piso sobe em degraus, nunca desce.
