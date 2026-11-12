export interface RecommendedSkill {
  id: string;
  name: string;
  description: string;
  url: string;
  sourceType: "github" | "url";
  source: string;
  skillId: string;
  installCommand?: string;
  tags: string[];
  note?: string;
  /** False when the project ships its own installer instead of a skill file. */
  installable?: boolean;
}

/**
 * Curated external skills. Installation reuses the Studio preview/apply
 * pipeline; the engine or CLI of each project remains the user's choice.
 */
export const RECOMMENDED_SKILLS: RecommendedSkill[] = [
  {
    id: "caveman",
    name: "Caveman",
    description:
      "Respostas ultracompactas que cortam o texto pela metade sem perder fatos técnicos.",
    url: "https://github.com/JuliusBrussee/caveman",
    sourceType: "url",
    source:
      "https://raw.githubusercontent.com/JuliusBrussee/caveman/main/skills/caveman/SKILL.md",
    skillId: "caveman",
    tags: ["economia de tokens", "estilo", "pt-BR"],
  },
  {
    id: "ponytail",
    name: "Ponytail",
    description:
      "Modo de engenharia sênior preguiçoso: a menor mudança que resolve, sem over-engineering.",
    url: "https://github.com/DietrichGebert/ponytail/blob/main/skills/ponytail/SKILL.md",
    sourceType: "url",
    source:
      "https://raw.githubusercontent.com/DietrichGebert/ponytail/main/skills/ponytail/SKILL.md",
    skillId: "ponytail",
    tags: ["simplicidade", "refatoração", "review"],
  },
  {
    id: "implement",
    name: "Implement",
    description:
      "Implementa uma spec ou tarefa de ponta a ponta, com testes e verificação.",
    url: "https://github.com/mattpocock/skills/blob/main/skills/engineering/implement/SKILL.md",
    sourceType: "github",
    source: "mattpocock/skills",
    skillId: "implement",
    installCommand: "npx skills add mattpocock/skills --skill implement",
    tags: ["implementação", "entrega", "testes"],
  },
  {
    id: "code-review",
    name: "Code Review",
    description:
      "Revisão de código focada em bugs, regressões, legibilidade e risco de merge.",
    url: "https://github.com/mattpocock/skills/blob/main/skills/engineering/code-review/SKILL.md",
    sourceType: "github",
    source: "mattpocock/skills",
    skillId: "code-review",
    installCommand: "npx skills add mattpocock/skills --skill code-review",
    tags: ["review", "qualidade", "merge"],
  },
  {
    id: "diagnosing-bugs",
    name: "Diagnosing Bugs",
    description:
      "Diagnóstico sistemático de falhas antes de propor correções, com hipóteses e evidências.",
    url: "https://github.com/mattpocock/skills/blob/main/skills/engineering/diagnosing-bugs/SKILL.md",
    sourceType: "github",
    source: "mattpocock/skills",
    skillId: "diagnosing-bugs",
    installCommand: "npx skills add mattpocock/skills --skill diagnosing-bugs",
    tags: ["debug", "investigação", "evidência"],
  },
  {
    id: "codebase-design",
    name: "Codebase Design",
    description:
      "Desenho de módulos e fronteiras com foco em evolução, coesão e baixo acoplamento.",
    url: "https://github.com/mattpocock/skills/blob/main/skills/engineering/codebase-design/SKILL.md",
    sourceType: "github",
    source: "mattpocock/skills",
    skillId: "codebase-design",
    installCommand: "npx skills add mattpocock/skills --skill codebase-design",
    tags: ["arquitetura", "módulos", "acoplamento"],
  },
  {
    id: "grilling",
    name: "Grilling",
    description:
      "Perguntas incisivas para stress-testar uma proposta antes de implementar.",
    url: "https://github.com/mattpocock/skills/blob/main/skills/productivity/grilling/SKILL.md",
    sourceType: "github",
    source: "mattpocock/skills",
    skillId: "grilling",
    installCommand: "npx skills add mattpocock/skills --skill grilling",
    tags: ["decisão", "review", "perguntas"],
  },
  {
    id: "tdd",
    name: "TDD",
    description:
      "Ciclos vermelho/verde/refatoração com testes que provam o comportamento antes da implementação.",
    url: "https://github.com/mattpocock/skills/blob/main/skills/engineering/tdd/SKILL.md",
    sourceType: "github",
    source: "mattpocock/skills",
    skillId: "tdd",
    installCommand: "npx skills add mattpocock/skills --skill tdd",
    tags: ["testes", "TDD", "qualidade"],
  },
  {
    id: "improve-codebase-architecture",
    name: "Improve Codebase Architecture",
    description:
      "Encontra acoplamentos e propõe uma arquitetura mais simples sem parar a entrega.",
    url: "https://github.com/mattpocock/skills/blob/main/skills/engineering/improve-codebase-architecture/SKILL.md",
    sourceType: "github",
    source: "mattpocock/skills",
    skillId: "improve-codebase-architecture",
    installCommand:
      "npx skills add mattpocock/skills --skill improve-codebase-architecture",
    tags: ["arquitetura", "refatoração", "dívida"],
  },
  {
    id: "grill-me",
    name: "Grill Me",
    description:
      "Entrevista técnica para expor premissas, lacunas e riscos escondidos em um plano.",
    url: "https://github.com/mattpocock/skills/blob/main/skills/productivity/grill-me/SKILL.md",
    sourceType: "github",
    source: "mattpocock/skills",
    skillId: "grill-me",
    installCommand: "npx skills add mattpocock/skills --skill grill-me",
    tags: ["planejamento", "entrevista", "risco"],
  },
  {
    id: "impeccable",
    name: "Impeccable",
    description:
      "Vocabulário de design para agentes: critique, polish, layout, tipografia e auditoria de UI.",
    url: "https://impeccable.style/#downloads",
    sourceType: "github",
    source: "pbakaus/impeccable",
    skillId: "impeccable",
    tags: ["design", "UI/UX", "auditoria"],
    installable: false,
    note: "O Impeccable tem instalador e engine próprios. Use o site oficial; a skill isolada não inclui o engine.",
  },
];
