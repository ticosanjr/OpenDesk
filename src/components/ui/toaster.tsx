import { useEffect } from "react";
import { Link } from "react-router-dom";
import { CheckCircle2, Info, TriangleAlert, X, XCircle } from "lucide-react";
import { useToastStore, type Toast, type ToastTone } from "@/store/toast-store";
import { cn } from "@/lib/utils";

const TONE_STYLES: Record<ToastTone, string> = {
  success: "border-emerald-500/30 bg-emerald-950/80 text-emerald-100",
  error: "border-red-500/30 bg-red-950/80 text-red-100",
  warning: "border-amber-500/30 bg-amber-950/80 text-amber-100",
  info: "border-blue-500/30 bg-blue-950/80 text-blue-100",
};

const TONE_ICONS = {
  success: CheckCircle2,
  error: XCircle,
  warning: TriangleAlert,
  info: Info,
} as const;

export function Toaster() {
  const toasts = useToastStore((state) => state.toasts);
  return (
    <div
      className="pointer-events-none fixed bottom-4 right-4 z-[70] flex w-full max-w-sm flex-col gap-2"
      aria-live="polite"
      aria-atomic="false"
    >
      {toasts.map((item) => (
        <ToastCard key={item.id} toast={item} />
      ))}
    </div>
  );
}

function ToastCard({ toast }: { toast: Toast }) {
  const dismiss = useToastStore((state) => state.dismiss);
  const Icon = TONE_ICONS[toast.tone];

  useEffect(() => {
    if (toast.duration <= 0) return;
    const handle = window.setTimeout(() => dismiss(toast.id), toast.duration);
    return () => window.clearTimeout(handle);
  }, [dismiss, toast.duration, toast.id]);

  return (
    <div
      role="status"
      className={cn(
        "pointer-events-auto flex items-start gap-2.5 rounded-xl border px-3.5 py-3 shadow-2xl backdrop-blur",
        TONE_STYLES[toast.tone],
      )}
    >
      <Icon size={16} className="mt-0.5 shrink-0 opacity-90" />
      <div className="min-w-0 flex-1">
        {toast.href ? (
          <Link
            to={toast.href}
            onClick={() => dismiss(toast.id)}
            className="block rounded outline-none hover:opacity-90 focus-visible:ring-2 focus-visible:ring-current/40"
          >
            <p className="text-[12px] font-medium leading-snug underline decoration-current/30 underline-offset-2">
              {toast.title}
            </p>
            {toast.description ? (
              <p className="mt-0.5 whitespace-pre-wrap text-[10px] leading-relaxed opacity-80">
                {toast.description}
              </p>
            ) : null}
          </Link>
        ) : (
          <>
            <p className="text-[12px] font-medium leading-snug">{toast.title}</p>
            {toast.description ? (
              <p className="mt-0.5 whitespace-pre-wrap text-[10px] leading-relaxed opacity-80">
                {toast.description}
              </p>
            ) : null}
          </>
        )}
        {toast.action ? (
          <button
            type="button"
            onClick={() => {
              toast.action?.onClick();
              dismiss(toast.id);
            }}
            className="mt-2 rounded-md border border-current/30 px-2 py-1 text-[10px] font-medium opacity-90 hover:opacity-100"
          >
            {toast.action.label}
          </button>
        ) : null}
      </div>
      <button
        type="button"
        aria-label="Fechar notificação"
        onClick={() => dismiss(toast.id)}
        className="shrink-0 rounded p-0.5 opacity-60 hover:opacity-100"
      >
        <X size={13} />
      </button>
    </div>
  );
}
