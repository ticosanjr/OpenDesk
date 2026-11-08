# Fábrica de Capacidades

Pacote para criar skills, agentes e comandos de alto nível com metodologia
eval-driven.

## Fluxo

1. Defina o objetivo, usuários, gatilhos e falhas reais.
2. Rode uma linha de base sem a capacidade.
3. Crie 3–5 evals de capability e 2 de regression.
4. Escreva a menor instrução que resolve as falhas.
5. Teste, leia as transcrições, categorize falhas e refine.
6. Passe pelo revisor e por aprovação humana antes de publicar.

## Pastas

- `capabilities/research/` — pesquisa e referências.
- `capabilities/evals/` — casos, rubricas e resultados.
- `capabilities/skills/` — pacotes de skill propostos.
- `capabilities/agents/` — agentes propostos.
- `capabilities/commands/` — comandos propostos.
- `capabilities/output/` — entregáveis finais e relatórios.

Nada é publicado em `.opencode` sem aprovação. Prefira propostas via
`opendesk-config` ou entregáveis em `capabilities/output/`.
