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
