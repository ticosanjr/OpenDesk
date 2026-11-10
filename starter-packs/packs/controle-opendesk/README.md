# Controle OpenDesk

Skill e comandos para os agentes operarem o próprio OpenDesk pelo MCP
`opendesk-control`: tarefas, execuções, rotinas, receitas, fila, artefatos e
itens de atenção.

O pacote é instalado automaticamente em novos workspaces. O MCP é registrado
em todos os workspaces no boot do app.

Regras principais:

- Rotinas e receitas nascem **desligadas**. Ativar exige pedido explícito do
  usuário.
- HITL, permissões e propostas de configuração são **somente leitura** para o
  agente; a decisão é humana.
- Mudanças em agentes, skills, comandos e `opencode.jsonc` passam pelo MCP
  `opendesk-config` como propostas com aprovação.
- Toda entrega durável usa `output/YYYY-MM/<slug>/`.

Remova o pacote pela página Pacotes sem apagar arquivos que você modificou.
