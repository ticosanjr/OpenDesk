# 0077 — Multi-node com sandbox, artefatos e rotação de token

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o worker remoto da 1.2.0 executava jobs CLI, mas ignorava a
  política de sandbox, não registrava artefatos e não permitia trocar o token.

## Decisão

1. **Política no claim.** `GET /api/nodes/next-job` devolve a política de
   sandbox efetiva do workspace. O worker aplica allowlist, `max_runtime_minutes`,
   env scrubbing e, em `strict`, bubblewrap — igual ao executor local.
2. **Diffs viram artefatos.** Após o run, o worker roda `git status --porcelain`
   e envia `files` no `complete`. A central valida caminhos (relativos, sem
   `..`), registra proveniência e publica `ArtifactCreated`.
3. **Rotação de token.** `POST /api/nodes/{id}/rotate` invalida o token anterior
   imediatamente e devolve o novo uma única vez; a UI permite copiar.
4. **AppImage inclui o daemon.** O build do `opendeskd` acontece antes do
   empacotamento e o binário é copiado para `usr/bin/opendeskd` no AppDir. O
   release também mantém o artefato avulso `opendeskd-<plataforma>`.
5. **Sandbox por workspace.** `sandbox_policy:<workspace>` sobrepõe a política
   global; a UI mostra o escopo efetivo. O worker sempre recebe o efetivo.
6. **Limites mantidos.** Jobs `opencode` continuam exigindo o daemon local e são
   recusados pelo worker; o workspace precisa estar montado no nó.

## Consequências

- Um job remoto agora respeita o mesmo contrato de isolamento e produz os mesmos
  artefatos observáveis que um job local.
- Tokens de nó podem ser rotacionados sem recriar o nó.
- Ainda não há mTLS entre central e worker; o token é a fronteira de confiança.
