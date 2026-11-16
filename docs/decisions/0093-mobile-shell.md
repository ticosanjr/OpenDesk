# 0093 — Shell mobile-first (PWA)

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** a PWA já tinha manifest e service worker, mas o shell era
  desktop-first: sidebar fixa de 64 px e topbar larga consumiam a tela em 390 px.

## Decisão

1. Em `<640px` a sidebar é ocultada e entra uma barra inferior fixa com quatro
   destinos: Caixa (Kanban/HITL), Chat, Capturar e Runs.
2. A barra respeita `env(safe-area-inset-bottom)` e o `viewport-fit=cover` foi
   adicionado ao HTML para iPhones com notch.
3. O `<main>` reserva a altura da barra (`pb-[calc(4rem+env(...))]`) para nenhum
   conteúdo ficar atrás dela.
4. Topbar compacta: seletor de workspace de 132 px, labels de busca/engine
   escondidos e gaps menores; orçamento/fila continuam apenas em `xl`.
5. Em `>=640px` a sidebar e o comportamento anteriores são preservados.
6. Playwright cobre a barra, a ausência da sidebar, o padding do main e a
   ausência de overflow horizontal em `/`, `/kanban`, `/runs`, `/chat`,
   `/artifacts` e `/settings` a 390×844; em 700 px o layout de tablet mantém a sidebar e os goldens existentes.

## Consequências

- O app é operável no celular sem depender de tabelas densas ou zoom.
- Nenhum golden desktop muda: a barra é `md:hidden` e a sidebar continua igual
  em 1440 px.
- A rota `/capture` continua fora do shell por ser uma janela de captura
  dedicada.
