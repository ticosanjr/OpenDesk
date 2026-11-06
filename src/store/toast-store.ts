import { create } from "zustand";

export type ToastTone = "success" | "error" | "warning" | "info";

export interface ToastAction {
  label: string;
  onClick: () => void;
}

export interface Toast {
  id: number;
  tone: ToastTone;
  title: string;
  description?: string;
  action?: ToastAction;
  /** Milliseconds before auto-dismiss; 0 keeps it until dismissed. */
  duration: number;
}

interface ToastStore {
  toasts: Toast[];
  push: (toast: Omit<Toast, "id" | "duration"> & { duration?: number }) => number;
  dismiss: (id: number) => void;
  clear: () => void;
}

let nextId = 1;

export const useToastStore = create<ToastStore>((set) => ({
  toasts: [],
  push: (toast) => {
    const id = nextId++;
    const duration = toast.duration ?? (toast.tone === "error" ? 0 : 4200);
    set((state) => ({
      toasts: [...state.toasts.slice(-4), { ...toast, id, duration }],
    }));
    return id;
  },
  dismiss: (id) =>
    set((state) => ({ toasts: state.toasts.filter((toast) => toast.id !== id) })),
  clear: () => set({ toasts: [] }),
}));

export function toast(input: {
  tone?: ToastTone;
  title: string;
  description?: string;
  action?: ToastAction;
  duration?: number;
}) {
  return useToastStore.getState().push({
    tone: input.tone ?? "info",
    title: input.title,
    description: input.description,
    action: input.action,
    duration: input.duration,
  });
}
