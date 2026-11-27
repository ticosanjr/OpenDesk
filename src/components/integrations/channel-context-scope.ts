import type { ChannelMemoryScope } from "@/lib/types";

export const SCOPE_LABEL: Record<ChannelMemoryScope, string> = {
  chat: "Chat",
  normal: "Normal",
  both: "Ambos",
  none: "Nenhuma",
};

export const SCOPE_HINT: Record<ChannelMemoryScope, string> = {
  chat: "Usa apenas memórias marcadas para chat.",
  normal: "Usa apenas memórias de execuções normais.",
  both: "Usa memórias de chat e normais.",
  none: "Não injeta memória pessoal neste grupo.",
};
