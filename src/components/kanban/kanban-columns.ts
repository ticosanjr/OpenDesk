import {
  AlertTriangle,
  CheckCircle2,
  Inbox,
  Loader2,
  XCircle,
  type LucideIcon,
} from "lucide-react";

export type WorkflowColumn = {
  id: string;
  label: string;
  tone: "default" | "blue" | "green" | "yellow" | "red" | "purple";
  icon: LucideIcon;
  states: string[];
  dropStatus: string;
  description: string;
};

export const KANBAN_COLUMNS: WorkflowColumn[] = [
  {
    id: "QUEUE",
    label: "Fila",
    tone: "blue",
    icon: Inbox,
    states: ["READY", "QUEUED", "HELD"],
    dropStatus: "TODO",
    description: "Prontas, na fila ou em espera. Iniciam automaticamente.",
  },
  {
    id: "RUNNING",
    label: "Em execução",
    tone: "purple",
    icon: Loader2,
    states: ["RUNNING"],
    dropStatus: "IN_PROGRESS",
    description: "Rodando de verdade, com job e run ativos.",
  },
  {
    id: "ATTENTION",
    label: "Atenção",
    tone: "yellow",
    icon: AlertTriangle,
    states: ["NEEDS_APPROVAL", "WAITING_INPUT", "BLOCKED"],
    dropStatus: "BLOCKED",
    description: "Precisa de você: aprovação, pergunta, permissão ou falha.",
  },
  {
    id: "DONE",
    label: "Concluído",
    tone: "green",
    icon: CheckCircle2,
    states: ["DONE"],
    dropStatus: "DONE",
    description: "Execução concluída.",
  },
  {
    id: "CANCELLED",
    label: "Cancelado",
    tone: "default",
    icon: XCircle,
    states: ["CANCELLED"],
    dropStatus: "CANCELLED",
    description: "Execução interrompida.",
  },
];
