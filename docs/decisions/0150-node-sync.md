# 0150 — D2: sync automático no worker remoto

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** o `opendesk node run` exigia que o workspace estivesse montado
  no nó; sem mount o worker falhava.

## Decisão

1. Novos endpoints autenticados por token de nó:
   - `POST /api/nodes/jobs/{id}/sync` recebe o manifest local e devolve um
     bundle delta (`bundle_id`, manifest do controller, delta);
   - `GET /api/nodes/jobs/{id}/sync/{bundle_id}` baixa o `.tar.zst`.
2. O worker, com `--sync` ou quando o caminho do controller não existe
   localmente, usa um workdir (`--workdir` ou temporário) e:
   - calcula o manifest local;
   - baixa/aplica o delta;
   - executa o harness no workdir.
3. Ao concluir, o worker calcula o delta pós-run e envia `content_base64` dos
   arquivos adicionados/alterados e a lista de removidos no `complete`.
4. O controller grava o conteúdo no workspace com validação de path e recusa
   symlink; depois registra a proveniência do artefato.
5. O bundle é validado por `workspace_sync::validate_bundle_path` e aplicado
   por `workspace_sync::apply`.

## Consequências

- Um nó sem mount executa e devolve artefatos.
- O primeiro job de um workspace envia o conteúdo completo; os seguintes usam
  delta.
- Arquivos muito grandes viajam em base64 no JSON de conclusão; uma próxima
  fatia pode trocar por upload binário multipart.
- O patch git deixa de ser a única fonte de conteúdo para nós remotos.
