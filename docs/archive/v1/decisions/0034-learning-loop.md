# ADR 0034 — Loop de aprendizado leve baseado em lições

## Contexto

O OpenDesk já registra runs, erros classificados, custos, sessões, artefatos e
propostas de configuração com aprovação humana. Faltava transformar esses sinais
em conhecimento operacional sem introduzir um pipeline pesado de ML, embeddings
ou dependências externas.

## Decisão

1. **Lições determinísticas, não inferência.** O módulo `learning` observa runs
   concluídos e agrupa padrões por fingerprint estável:
   - falhas usam o código de erro já classificado (`classify_run_error`) +
     agente + modelo + mensagem normalizada;
   - sucessos são opcionais e agrupam agente + modelo.
2. **Captura idempotente.** A tabela `learning_run_captures` garante que cada run
   seja observado no máximo uma vez; varreduras manuais não inflam ocorrências.
3. **HITL obrigatório.** Promover uma lição cria uma `config_proposal` do tipo
   `SKILL` no fluxo existente. O arquivo só é gravado no workspace quando o
   usuário aprova a proposta. O loop nunca instala nada sozinho.
4. **Sem prompts completos.** A evidência guarda apenas código de erro, mensagem
   truncada, agente, modelo, duração e custo. O conteúdo sensível fica de fora.
5. **Leve por padrão.** O aprendizado vem desligado para sucessos, usa uma única
   tabela de lições e não adiciona worker próprio: a captura roda após a
   liquidação do run e é best-effort.

## Consequências

- Zero dependências novas e custo de armazenamento proporcional a padrões, não a
  runs.
- O usuário controla captura, mínimo de ocorrências e promoção automática.
- A promoção automática, quando ligada, ainda passa por aprovação na proposta.
- O conhecimento promovido vira skill nativa em `.opencode/skills`, reutilizável
  por qualquer agente OpenCode.
- Uma futura busca semântica ou memória em camadas pode ser adicionada sobre as
  lições sem mudar o contrato atual.
