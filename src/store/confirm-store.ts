import { create } from "zustand";

export interface ConfirmOptions {
  title: string;
  description?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  tone?: "default" | "destructive";
}

interface ConfirmStore {
  options: ConfirmOptions | null;
  resolve: ((value: boolean) => void) | null;
  request: (options: ConfirmOptions) => Promise<boolean>;
  settle: (value: boolean) => void;
}

export const useConfirmStore = create<ConfirmStore>((set, get) => ({
  options: null,
  resolve: null,
  request: (options) =>
    new Promise<boolean>((resolve) => {
      get().resolve?.(false);
      set({ options, resolve });
    }),
  settle: (value) => {
    get().resolve?.(value);
    set({ options: null, resolve: null });
  },
}));

export function confirmAction(options: ConfirmOptions): Promise<boolean> {
  return useConfirmStore.getState().request(options);
}
