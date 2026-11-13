# Verificação — fases 0 a 5

Estado verificado na árvore local para o release **0.52.0**. O gate completo
foi executado antes da publicação; as pendências manuais continuam listadas
abaixo.

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
- Agente `opendesk` incluído no pacote base 1.1.0; cuidado rápido o usa quando
  disponível e cai para `build` em workspaces antigos.
- Tarefas de instalação usam cuidado padrão e continuam rastreáveis.

## Verificações manuais pendentes

Estas dependem de credenciais/dispositivos e não podem ser automatizadas aqui:

1. **SMTP Gmail real:** conectar App Password, testar conexão e enviar um
   e-mail de receita para destinatário explícito e para o e-mail padrão.
2. **Signal real:** vincular um celular escaneando o QR e enviar/receber uma
   mensagem pela bridge.
3. **Windows/macOS:** gerar instaladores e validar assinatura/updater nas
   máquinas nativas (`docs/platform-builds.md`).
4. **Contrato Google:** executar com a conta dedicada (`docs/google-contract.md`).

## Próximo passo

Rodar o gate completo uma última vez após as verificações manuais e publicar a
próxima versão com `scripts/release.sh`.
