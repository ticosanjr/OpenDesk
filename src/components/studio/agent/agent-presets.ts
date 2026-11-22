import type { AgentFile, StudioScope } from "@/lib/types";

export const ACTIONS = [
  "*",
  "edit",
  "shell",
  "read",
  "glob",
  "grep",
  "webfetch",
  "websearch",
  "subagent",
  "skill",
];

export const EFFECTS = ["allow", "ask", "deny"];

export const AVATAR_PRESETS = [
  "🤖",
  "🧠",
  "🛡️",
  "🧾",
  "📊",
  "✍️",
  "🔍",
  "⚙️",
  "🚀",
  "💡",
  "🎯",
  "📚",
  "🦉",
  "🧩",
  "🕵️",
  "🧑‍💻",
  "📈",
  "🧪",
  "🗂️",
  "🔧",
];

export const COLOR_PRESETS = [
  "#60a5fa",
  "#34d399",
  "#fbbf24",
  "#f87171",
  "#a78bfa",
  "#22d3ee",
  "#fb923c",
  "#f472b6",
];

export function blankAgent(scope: StudioScope): AgentFile {
  return {
    id: "",
    scope,
    path: "",
    name: "",
    avatar: "🤖",
    description: "",
    mode: "subagent",
    model: "",
    color: "#60a5fa",
    router_policy: null,
    hidden: false,
    steps: null,
    disabled: false,
    permissions: [],
    system: "",
    sha256: "",
  };
}

export function initials(value: string): string {
  const chars = Array.from(value.trim());
  return chars.slice(0, 2).join("") || "?";
}

export const inputClass =
  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[13.5px] text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-blue-500/60 disabled:cursor-not-allowed";
