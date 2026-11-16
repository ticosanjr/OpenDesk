# 0070 — Sandbox de CLI e computer-use (onda 7)

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** harnesses CLI executam código no host e a v1.0 precisa de
  isolamento explícito e de uma ponte para automação de GUI, sem prometer mais
  isolamento do que o sistema entrega.

## Decisão

1. **Política de sandbox global.** `sandbox_policy` tem três modos:
   - `off`: comportamento anterior;
   - `workspace`: cwd fixo no workspace, allowlist opcional de comandos e
     remoção de credenciais comuns do ambiente;
   - `strict`: bubblewrap com sistema read-only, workspace read-write, `/tmp`
     isolado e rede opcional.
2. **Sem degradação silenciosa.** Se `strict` for escolhido e `bwrap` não
   existir, o run falha com `sandbox_unavailable`. O status da API mostra se o
   bwrap está disponível.
3. **Limites por execução.** `max_runtime_minutes` substitui o timeout padrão de
   30 minutos quando configurado.
4. **Computer-use é uma ponte fina.** `computer_sessions` registra status,
   contagem e resumo das ações. Capturas usam `grim` (Wayland) ou
   `import`/`scrot` (X11); entrada usa `ydotool` (Wayland) ou `xdotool` (X11).
5. **Aprovação humana opcional.** A sessão nasce `PENDING` quando
   `require_approval=true` e só aceita ações após aprovação explícita. Sessões
   encerradas não executam nada.
6. **Privacidade.** Texto digitado nunca é persistido: o resumo guarda apenas o
   tipo e o comprimento. Capturas ficam em
   `output/YYYY-MM/computer/<session>.png`.
7. **UI.** Configurações → Sandbox mostra a política, o status do bwrap, as
   capacidades do host, sessões, captura e controles de entrada.

## Consequências

- O usuário escolhe o nível de isolamento com expectativa correta; não há
  sandbox "mágico" em plataformas sem suporte.
- Computer-use depende de ferramentas do host, por design; a UI explica o que
  falta em vez de falhar em silêncio.
- A política é global nesta versão; perfis por workspace/agente podem reusar a
  mesma tabela de settings sem migração.
