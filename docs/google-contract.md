# Contrato Google (conta dedicada)

O contrato real roda contra uma conta Google dedicada, nunca contra a conta
pessoal de quem desenvolve. Ele é opt-in e não entra no gate: depende de OAuth
concluído em uma instância local do OpenDesk.

## Preparação

1. Crie a conta dedicada (ex.: `opendesk-contrato@empresa.com`) e compartilhe
   com ela um Google Doc de teste, se quiser validar Docs.
2. No Google Cloud Console, crie um OAuth Client do tipo **Desktop**.
3. Em OpenDesk → Integrações → Google, informe client id/secret, conecte a
   conta dedicada e confirme os scopes exibidos.
4. Reconecte contas antigas: a busca de threads Gmail passou a exigir
   `gmail.readonly`, adicionado depois do `gmail.send`.

## Execução

```bash
# app rodando e Google conectado no workspace
OPENDESK_CONTRACT_GOOGLE=1 \
OPENDESK_CONTRACT_DOC_ID=<id-do-doc> \
npm run contract:google
```

Variáveis opcionais:

| Variável | Uso |
|---|---|
| `OPENDESK_CONTRACT_SKIP` | `gmail,calendar,tasks,docs` para pular serviços |
| `OPENDESK_CONTRACT_WORKSPACE_ID` | workspace específico |
| `OPENDESK_DATA_DIR` | diretório do app (default `~/.local/share/opendesk`) |
| `OPENDESK_CONTRACT_BIN` | binário do OpenDesk |

## O que é validado

- Status da integração e presença dos scopes de Drive, Gmail (send/readonly),
  Calendar (readonly) e Tasks.
- `tools/list` do MCP Google com as ferramentas esperadas.
- Drive: `gdrive_search_files` (somente leitura).
- Gmail: `gmail_search_threads` com query vazia (somente leitura).
- Calendar: `gcalendar_list_events` em uma janela de 30 dias.
- Tasks: `gtasks_list` (somente leitura).
- Docs: `gdocs_read_document` quando `OPENDESK_CONTRACT_DOC_ID` é informado;
  caso contrário o passo é ignorado com aviso.
- Cada processo MCP novo força a renovação do access token a partir do refresh
  token, então rodar o contrato duas vezes cobre o caminho de renovação.

## Fluxos de erro

- Sem refresh token, `access_token` falha com
  `Google Workspace is not connected for this workspace` (teste unitário).
- Refresh rejeitado retorna 400 com instrução de reconectar a conta.
- Chamadas de serviço sem scope retornam `isError` no MCP e o contrato aponta
  qual serviço falhou.
- Desconectar revoga o token remoto quando possível e sempre limpa o segredo
  local (teste unitário).

## Limites

- O contrato é read-only; envio de e-mail, criação de documentos/eventos e
  escrita em Tasks continuam cobertos por testes de payload e pela suíte de
  features, não pela conta dedicada.
- A conta dedicada e as credenciais ficam fora do repositório; não há CI.
