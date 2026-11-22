import { FolderOpen, Github, Globe, Package, Plug, Terminal } from "lucide-react";

export type SourceType = "catalog" | "github" | "url" | "local" | "mcp_json" | "npm";

export const SOURCES: Array<{
  id: SourceType;
  label: string;
  icon: typeof Github;
  hint: string;
}> = [
  {
    id: "catalog",
    label: "Catálogo MCP",
    icon: Plug,
    hint: "Presets prontos: filesystem, git, fetch, memory, thinking e Playwright.",
  },
  {
    id: "github",
    label: "GitHub",
    icon: Github,
    hint: "owner/repo, owner/repo@ref ou github:owner/repo.",
  },
  {
    id: "url",
    label: "URL",
    icon: Globe,
    hint: "Link direto para SKILL.md, agente, comando, zip ou JSON de MCP.",
  },
  {
    id: "local",
    label: "Pasta/arquivo",
    icon: FolderOpen,
    hint: "Selecione um arquivo .md ou uma pasta com .opencode.",
  },
  {
    id: "mcp_json",
    label: "MCP JSON",
    icon: Terminal,
    hint: "Cole o objeto do servidor MCP.",
  },
  {
    id: "npm",
    label: "npm",
    icon: Package,
    hint: "Nome do pacote npm que expõe um servidor MCP.",
  },
];

export const inputClass =
  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[14px] text-zinc-100 outline-none placeholder:text-zinc-500 transition-colors hover:border-zinc-700 focus:border-blue-500/60";
