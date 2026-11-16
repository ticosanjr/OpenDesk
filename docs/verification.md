# Verificação — fases 0 a 5

Estado verificado na árvore local para o release **0.55.0**. Este release usou a
verificação proporcional; o último gate completo registrado foi o de 0.53.0. As
pendências manuais continuam listadas abaixo.

## Comandos e resultados

| Verificação | Resultado |
|---|---|
| `scripts/gate.sh` | **verde** (tsc, ESLint, Vitest+cobertura, fmt, clippy, Rust tests, cobertura Rust, build, Playwright, goldens) |
| `npm run test:features` | **74/74 (100%)** |
| `npm run audit:prod` | **0 vulnerabilidades** |
| Vitest | **222 testes** |
| Playwright | **93 testes** |
| Rust | **397 testes** |
| Goldens visuais | **49 imagens** estáveis |
| Cobertura Rust | **64,81% total / 71,14% core** (ratchets 64,5% / 70,8%) |

## Aceitação por fase

### Fase 0 — entrada, estado e layout
- Digitação rápida na busca preserva todos os caracteres (teste dedicado).
- Status de execução usa o mesmo ícone/cor em Execuções, Run log, Calendário e
  detalhes; cancelada é cinza com X.
- Sidebar, botões do card, composers, scroll automático e ordem dos cards
  validados por testes e goldens.

### Fase 1 — Kanban/Fila × Execuções
- `/api/tasks?scope=work` esconde automações do Kanban; fila só mostra trabalho.
- Execuções mostram tudo e filtram por trabalho/automações/rotinas/receitas.
- E2E `kanban-origins` cobre os dois lados.

### Fase 2 — catálogo, Studio, receitas, downloads e HTML
- Favoritos v2 para pack, skill, template, workspace, agente, comando, MCP e
  recurso externo.
- Recomendados aparecem na visão geral/busca e instalam por tarefa rastreável.
- Studio resume escopos Global/Workspace/Efetivo.
- Receitas têm estado vazio correto, nome no fluxo e YAML colapsável.
- Downloads usam pasta configurada ou diálogo nativo; HTML permite scripts por
  visualização; arquivos têm ações de abrir arquivo/pasta.

### Fase 3 — I/O, arquivos de entrada, notificações, SMTP e Signal
- Folder Watch com espera, retry, vazios, existentes e padrões ignorados.
- Timeline expõe arquivos de entrada e o run log permite abri-los.
- Notificações incluem tarefa, agente, duração, custo, tokens e falha.
- SMTP força STARTTLS em 587 com credenciais, SSL em 465 e normaliza App
  Password; sink de e-mail usa destinatário do workspace quando vazio.
- Signal lê o URI de vínculo de stdout/stderr; teste com CLI falso cobre stderr.

### Fase 4 — agente e cuidado
- `care_level` persiste e altera as instruções do prompt.
- Agente `opendesk` incluído no pacote base 1.2.0; cuidado rápido o usa quando
  disponível e cai para `build` em workspaces antigos.
- Tarefas de instalação usam cuidado padrão e continuam rastreáveis.

## Integração Telegram e atalho do AppImage (2026-11-14)

- Telegram: configuração, token criptografado, allowlist por ID/@username,
  long polling, `/start`, chat com agente, `aprovar`/`negar`, HITL e avisos de
  rotinas; migração 0035 e canal `TELEGRAM` no ledger.
- AppImage: `.desktop` automático com `StartupWMClass=Opendesk`, ícone absoluto
  e botão de reparo em Configurações → Sobre.
- Testes automatizados: 410 Rust e 222 Vitest. O bridge real depende de um bot
  criado no `@BotFather` e do app em execução.

## Pós-0.53.0 — chat operacional e HITL no Telegram

- `npm run verify` verde; 417 testes Rust e 222 Vitest.
- E2E `chat-scroll` valida rolagem interna e ausência de sobreposição com o
  composer em 1100x700 e 1440x900.
- `callback_query`, botões inline, `#código` e `artifact_read` têm cobertura
  unitária; o bot real e a aprovação de uma tarefa HITL seguem manuais.
- Pacote base `controle-opendesk` 1.2.0: workspaces antigos precisam de
  **Pacotes → Atualizar todos**.
- Goldens do chat (`chat-1024`, `chat-1440`) e `dashboard-700` atualizados
  intencionalmente; o dashboard estava defasado pelo rótulo “inclui automações”.

## Pós-0.54.0 — Folder Watch e turno do chat

- Folder Watch: fingerprint persistente e retry apenas para falha de início;
  testes de estado, skip, fingerprint e conciliação de runs antigos.
- Chat: polling até `idle`/estabilidade e parsing somente do turno atual; testes
  cobrem resposta antiga, turno novo e fake OpenCode.
- `npm run verify` verde; **421 testes Rust** e 222 Vitest. Teste manual
  pendente com bot real e uma pasta monitorada.

## Pós-0.55.0 — acabamento operacional (fase 1)

- Dashboard: overview de integrações; Busca: atividades recentes; Receitas:
  execução clicável, exclusão com segundo clique e splitter YAML.
- Abrir pasta: comando Rust valida workspaces/app; escopos do opener incluem
  `$HOME` e `$TEMP`.
- Rate limit: GETs read-only do Studio/agentes/modelos não contam; `apiGet`
  repete uma vez em 429.
- Verificação: `npm run verify` verde, 424 testes Rust, 223 Vitest, E2E
  `search-recent` e `npm run test:visual` verdes.

## Pós-0.55.0 — contexto pessoal e prompt (fase 2)

- Personal Context com escopos workspace/global, CRUD e injeção automática.
- Prompt preview com seções e origem; aba Configurações → Prompt.
- Migração 0037; testes de CRUD, render, prompt preview e parsing de seções.
- Verificação: `npm run verify` verde; 427 testes Rust e 223 Vitest; golden de
  Aprendizado atualizado.

## Pós-0.55.0 — arquivos no Telegram, anexos e watchers (fase 3)

- Entrada de documentos, fotos, áudio, voz e vídeo no Telegram, com limite de
  20 MB e salvamento em `input/telegram/`.
- Sink `TELEGRAM_MSG` e entregas de rotinas enviam artefatos com `sendDocument`.
- Watchers de conclusão por tarefa em Telegram/Signal, inclusive em falhas.
- Migração 0038; API `GET/PUT /api/tasks/{id}/watchers`; testes de parsing de
  arquivos, sanitização de nome, repositório e API.

## Pós-0.55.0 — HITL 2.0 (fase 4)

- Estruturas APPROVAL, SINGLE_CHOICE, MULTI_CHOICE e TEXT em tarefas e
  receitas, com payload opcional e mapeamento dos tipos legados.
- `ask_user` com `kind`; resposta com `decision`; validação por estrutura.
- Modal global de HITL em qualquer página, notificação nativa, Telegram e
  Signal.
- Migração 0039; ADR 0055; testes de API, parsing remoto e componente.

## Pós-0.55.0 — tela única de execução (fase 5)

- Modal com modo, agente, modelo, effort, comando/skill e instrução avulsa.
- `prompt_append` por execução, sem persistir na tarefa.
- Status ao vivo e parada no mesmo lugar; aprovação de plano reaproveita a tela.
- ADR 0056; testes do append e do modal.

## Pós-0.56.0 — chat com arquivos e artefatos de shell

- Turno do chat exige texto + idle; ferramenta sem texto não encerra mais.
- Envio de arquivo por thread (`/api/chat/threads/{id}/files` e
  `chat_send_file`) com validação de workspace e canais Telegram/Signal.
- Varredura do diretório de saída registra arquivos criados por shell.
- ADR 0057; testes de turno com ferramenta, envio por caminho e varredura.

## Pós-0.57.0 — chat como execução, prompts e prévias

- Chat/Telegram: `chat_turns`, runs `source=chat` para turnos com ferramenta ou
  arquivo, artefatos com canal, stop e ausência do corte de 5 minutos.
- Observabilidade por origem; templates de prompt editáveis; preview
  reconstruído com camada OpenCode e preview do chat por canal.
- Prévia HTML recarregável, JSON formatado, contexto pessoal sem título e
  detecção ampliada de ferramentas externas.
- Migração 0040; ADRs 0058–0060.

## Pós-0.58.0 — abas de prompt e memória via MCP

- Configurações → Prompt com três abas e previews na ordem Seções/OpenDesk/
  OpenCode/Final reconstruído.
- Templates adicionais: memória, correção de receita e teste de modelo.
- `learning_save` e `personal_context_save` com citação explícita; migração
  0041; ADR 0061.

## Pós-0.59.0 — v1.0 onda 0

- Contrato de adapter, eventos normalizados e registry.
- `opendeskd` headless e encerramento gracioso.
- Migração 0042 com schemas de handoff, ticket, canal, memória, workers e
  runtime events.
- Registro de variáveis de prompt; ADR 0062.

## Pós-0.59.0 — v1.0 onda 1 (parte 1)

- Daemon com host configurável, PWA estática e login por sessão revogável.
- CLI `opendesk chat`, `approvals` e `diff`, além de serviços de exemplo.
- ADR 0063.
- Web Push/VAPID: migração 0044, API, dispatcher, painel do PWA e service
  worker; testes de chaves, CRUD e endpoints.

## Pós-0.59.0 — v1.0 onda 2

- Harnesses OpenCode/Claude Code/Codex/Pi/Hermes, normalização e execução
  compartilhada.
- Migração 0045, API `/api/adapters`, UI de harnesses e seleção no DSH.
- Testes de normalização, spawn real de CLI e endpoints.

## Pós-0.59.0 — v1.0 onda 3

- Rotas de canal/trust, tickets idempotentes, webhook autenticado e WhatsApp
  estrutural.
- Testes de rota, token, confiança, deduplicação e criação de tarefa.
- UI Integrações → Canais e tickets.

## Pós-0.59.0 — v1.0 onda 4

- Handoffs com estados, aceite/rejeição, guard de orçamento, conclusão por run,
  MCP e UI.
- Testes de transições, criação de tarefa, recusa e orçamento.

## Pós-0.59.0 — v1.0 onda 5

- Memória estruturada, namespaces/ACL, busca, validade e injeção no prompt.
- MCP de memória e avaliação de skills com UI.
- Testes de ACL, busca, expiração e API.

## Pós-0.59.0 — v1.0 onda 6

- MCP Hub: inventário, catálogo, teste, toggle e remoção.
- Testes de instalação/toggle/delete, catálogo e comandos.

## Pós-0.59.0 — v1.0 onda 7

- Sandbox de CLI, status do bwrap e computer-use com aprovação.
- Testes de política, allowlist, sessões e transições.

## Pós-0.59.0 — v1.0 onda 8

- Voz (STT/TTS) e protocolo multi-node com claim/heartbeat/complete.
- Testes de settings/capacidades, registro de nó, token, claim e conclusão.

## 1.0.0 — hardening e fechamento

- 513 testes Rust, 234 Vitest, ESLint/tsc e goldens visuais verdes.
- Cabeçalhos de segurança, CSP da PWA, `/readyz` e retenção de sessões.
- Release notes em `docs/releases/1.0.0.md`; ADR 0072.

## 1.1.0 — passkeys, bootstrap e TUI

- Passkeys WebAuthn (origem, cerimônias, credenciais e login).
- Bootstrap compartilhado e TUI do CLI.
- Testes de origem, cerimônias, endpoints públicos/autenticados e TUI.

## 1.2.0 — worker remoto e distribuição do daemon

- Worker `opendesk node run` (claim, heartbeat, execução e complete).
- Release publica `opendeskd-<plataforma>`.
- Testes do mapeamento de harnesses e da suíte completa.

## Pós-1.2.0 — WhatsApp Cloud API e multi-node

- Webhook Meta (challenge + assinatura), normalização e criação de ticket.
- Sandbox propagado ao worker, artefatos remotos e rotação de token.
- Sandbox por workspace e `opendeskd` no AppImage.
- Testes de assinatura, extração Meta, rotas, artefatos e rotação.

## Pós-1.2.0 — worker OpenCode

- Worker remoto suporta OpenCode local via REST/SSE, com timeout e interrupt.
- Testes de parsing SSE e mapeamento de sessão.

## Pós-1.2.0 — mídia e pacote

- Extração de mídia Meta e extensões seguras cobertas por testes.
- Injeção de `opendeskd`/systemd em `.deb` validada manualmente com `dpkg-deb`.

## Fase 0/1 — receitas duráveis e schemas

- Nós persistentes, resume/cancel/retry e timeline.
- Contratos JSON Schema com extração e retry.
- Testes de store, parser, API e UI.

## Fase 2 — MCP dinâmico

- Filtragem por fase, descoberta sob demanda e três novas tools.
- Testes de visibilidade e busca.

## Fase 1 — condicionais de receita

- `when` avaliado antes do passo; nós `SKIPPED` e retomada.
- Testes de expressões e render.

## Fase 4 — memória

- Arquivo de expiradas, decaimento e deduplicação exata.
- Testes de consolidação/arquivo.

## Fase 4 — pack pessoal

- Manifesto, skill, agente, comando e receita validados pelo starter pack.
- Testes de packs verdes.

## Fase 4 — doctor live

- Sondas de daemon, harnesses, SMTP, Google, Telegram, Signal, WhatsApp e WebAuthn.
- Relatório humano/JSON com status por integração.

## Fase 1 — custo estimado CLI

- Estimativa de tokens/custo para harnesses sem usage nativo.
- Testes de tokenização, preços e modelos locais/desconhecidos.

## Sprint 1 — auto-attach Desktop↔daemon

- Probe de `api.json` + `/healthz` com timeout de 1s e apenas hosts locais.
- `api.json` corrompido degrada para modo embarcado.
- `server.pid` rejeita segundo processo e substitui PID obsoleto.
- Modo remoto usa o token do daemon e valida workspace antes de abrir caminho.

## Sprint 1 — write-buffer de heartbeats

- 50 heartbeats do mesmo job coalescidos em um único lote.
- 4 runs + 8 nós com 1.200 atualizações concorrentes, sem falha de lock.
- Presença de nós e lease de jobs atualizados após o flush.

## Sprint 1 — opendesk-core (primeira fatia)

- `cargo tree -p opendesk-core` sem Tauri.
- `cargo test --lib` executa shell e core.
- Contratos base reexportados sem mudar chamadas internas.

## Sprint 2 — receitas DAG

- Parser e validação de `branch`, `parallel` e `map` com limites.
- Execução de branch/parallel/map sem dispatch quando os passos estão desativados.
- Map com itens dinâmicos cria caminhos `mapa/<item>/<step>` e ordena resultados.
- Resume preserva passo DONE e restaura o output salvo.

## Sprint 4 — tools MCP dinâmicas

- Receitas habilitadas viram `recipe_<id>`; desabilitadas não aparecem.
- Agentes habilitados viram `agent_<id>`; desabilitados não aparecem.
- Nome sanitizado e busca em `mcp_discover_tools`.

## Sprint 3 — worktrees

- Git: prepare, diff com arquivo novo, merge aprovado e cleanup.
- Merge sem aprovação é rejeitado.
- Overlay sem Git copia, mostra diff e aplica alterações.

## Sprint 5 — shell mobile

- 390×844: barra inferior visível, sidebar oculta, main reserva 64 px.
- Rotas principais sem overflow horizontal no viewport de celular.
- 1440 px preserva sidebar e esconde a barra inferior.

## Sprint 6 — onboarding

- Checklist reflete workspace, motor, MCP, pacote base e integrações.
- Diálogo abre pela Topbar, verifica de novo e navega para a tela do item.
- E2E com workspace real valida root e botão Verificar.

## Sprint 6 — doctor live

- Probes reais de Google (refresh), Telegram (getMe), Signal (CLI) e WhatsApp (Graph).
- Sem configuração o status é not-configured; sem rede/token inválido é error.

## Sprint 2 — sub-receitas

- Chamada com `with` renderiza o trigger da sub-receita.
- Outputs ficam em `steps.<call>.outputs` sem colidir com o pai.
- Ciclo entre duas receitas é rejeitado antes da execução.

## Sprint 3 — sandbox container

- Modo container valida imagem e gera comando Docker/Podman sem shell.
- UI mostra runtime disponível e permite editar a imagem.
- Sem runtime, o run falha com conflito explícito.

## Sprint 3 — SessionContextPacket

- Pacote inclui plano, decisão e orçamento e é persistido por tarefa.
- Rota de inspeção devolve o pacote atual.
- Prompt preview/dispatch inclui a seção de continuidade.

## Sprint 2 — versionamento de receitas

- Duas edições geram duas versões; repetir o mesmo YAML não duplica.
- Diff da versão antiga para a atual mostra a mudança.
- Rollback restaura o conteúdo e registra uma nova versão.

## Sprint 2 — orçamento de sub-receita

- Budget inválido é rejeitado no parser.
- Estouro de limite falha após o step, com gasto e teto.
- Grupos propagam e somam o gasto no escopo pai.

## Verificações manuais pendentes

Estas dependem de credenciais/dispositivos e não podem ser automatizadas aqui:

1. **SMTP Gmail real:** conectar App Password, testar conexão e enviar um
   e-mail de receita para destinatário explícito e para o e-mail padrão.
2. **Signal real:** vincular um celular escaneando o QR e enviar/receber uma
   mensagem pela bridge.
3. **Telegram real:** criar bot no `@BotFather`, enviar `/start`, conversar com
   o agente, aprovar uma permissão/HITL e receber um aviso de rotina.
4. **Windows/macOS:** gerar instaladores e validar assinatura/updater nas
   máquinas nativas (`docs/platform-builds.md`).
5. **Contrato Google:** executar com a conta dedicada (`docs/google-contract.md`).
6. **Atalho do AppImage:** confirmar no Cinnamon que a barra de tarefas usa o
   mesmo ícone do tray após reintegrar o atalho.

## Próximo passo

Rodar o gate completo uma última vez após as verificações manuais e publicar a
próxima versão com `scripts/release.sh`.
