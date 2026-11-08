---
name: criação de skills
description: Cria e melhora SKILL.md com avaliação antes da documentação, descrição orientada a gatilhos e progressive disclosure.
---
# Criação de skills

## Quando usar
Ao transformar um workflow repetível, conhecimento de domínio ou conjunto de
scripts em uma skill reutilizável.

## Workflow

Copie e acompanhe:

```
Progresso:
- [ ] 1. Objetivo e falhas reais
- [ ] 2. Linha de base sem skill
- [ ] 3. Evals de capability e regression
- [ ] 4. Solução de referência
- [ ] 5. Draft mínimo do SKILL.md
- [ ] 6. Teste e leitura de transcrições
- [ ] 7. Refino por falha
- [ ] 8. Gate de revisão e aprovação
```

1. **Objetivo:** defina usuário, tarefa, gatilhos e não-objetivos.
2. **Baseline:** execute 3 tarefas reais sem a skill e registre falhas.
3. **Evals:** crie 3–5 casos de capability e 2 de regression; inclua casos em
   que a skill **não** deve disparar.
4. **Referência:** produza uma saída conhecida que passe na rubrica.
5. **Draft:** frontmatter com `name` e `description`; corpo conciso com workflow,
   checklist e exemplos. Mantenha abaixo de 500 linhas.
6. **Teste:** rode com pelo menos dois modelos ou configurações.
7. **Refino:** cada falha justifica uma mudança; corte instruções sem evidência.
8. **Gate:** peça revisão independente antes de publicar.

## Regras de frontmatter
- `name`: minúsculas, números e hífens; até 64 caracteres.
- `description`: terceira pessoa, o que faz + quando usar, termos de busca reais.
- Não use nomes reservados nem XML.

## Estrutura
- `SKILL.md`: guia curto.
- `references/*.md`: detalhe carregado sob demanda, um nível de profundidade.
- `scripts/`: operações determinísticas.
- `evals/`: casos, rubrica e resultados.

## Anti-padrões
- Documentar antes de saber qual falha a skill resolve.
- Descrição genérica como “ajuda com documentos”.
- Repetir conhecimento que o modelo já tem.
- Muitas opções sem default.
- Referências aninhadas ou nomes `file1.md`.
