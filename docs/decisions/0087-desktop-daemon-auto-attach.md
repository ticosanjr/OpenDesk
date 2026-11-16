# 0087 — Auto-attach do Desktop a um daemon externo

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o desktop sempre inicializava um backend embarcado, mesmo com
  `opendeskd` já ativo (systemd, container ou execução manual). Dois processos
  disputavam o SQLite, a porta da API e o `api.json`.

## Decisão

1. Antes de criar estado local, o desktop lê `api.json` e sonda `/healthz`
   com timeout de 1s. Apenas `127.0.0.1`, `localhost` e `::1` são aceitos e o
   token precisa estar presente.
2. Um `api.json` corrompido ou ilegível degrada para o modo embarcado em vez de
   impedir a inicialização.
3. A UI recebe o endpoint ativo (local ou remoto) pelo comando
   `get_api_endpoint` e usa o mesmo contrato REST/WS nos dois modos.
4. O shell passa a ter `ShellState::Embedded(AppState)` ou
   `ShellState::Remote(RemoteShell)`. Em modo remoto o desktop não abre o
   SQLite, não reserva porta e não escreve `api.json`.
5. `server.pid` fecha a corrida entre "nenhum daemon respondeu" e a criação do
   estado embarcado. No Linux a vivacidade é verificada por `/proc`; nos demais
   sistemas o arquivo é tratado como obsoleto depois que `/healthz` falha.
6. `opendeskd` também sonda `/healthz` e adquire o mesmo lock, então nem o
   daemon nem o desktop duplicam o backend.
7. A bandeja em modo remoto consulta `/api/queue/status` e
   `/api/daemon/status` a cada 5s e encaminha pause/resume/restart à API externa.
8. Abrir caminhos em modo remoto consulta `/api/workspaces` e valida o caminho
   localmente contra os diretórios do OpenDesk; o daemon não abre arquivos por
   procuração.

## Consequências

- Desktop e daemon compartilham a mesma verdade operacional sem sincronização
  manual de configuração.
- O modo remoto é deliberadamente somente localhost; execução em outra máquina
  continua sendo responsabilidade do protocolo de nós/mTLS.
- O lock é um PID best-effort: não substitui um supervisor de serviço, mas
  elimina duplicação acidental em inicializações concorrentes.
