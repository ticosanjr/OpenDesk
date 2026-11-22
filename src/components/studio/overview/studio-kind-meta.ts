import { BookOpen, Bot, Plug, Terminal } from "lucide-react";

export const KIND_META = {
  agent: { label: "Agente", icon: Bot },
  skill: { label: "Skill", icon: BookOpen },
  command: { label: "Comando", icon: Terminal },
  mcp: { label: "MCP", icon: Plug },
} as const;
