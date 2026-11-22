import {
  Clipboard,
  Loader2,
  RefreshCw,
  Search,
  Terminal,
  Undo2,
} from "lucide-react";
import { Button } from "@/components/ui/button";

export function EngineToolbar({
  busy,
  hasDiagnostics,
  onDetect,
  onRestart,
  onRollback,
  onDiagnostics,
  onRevealLogs,
}: {
  busy: string | null;
  hasDiagnostics: boolean;
  onDetect: () => void;
  onRestart: () => void;
  onRollback: () => void;
  onDiagnostics: () => void;
  onRevealLogs: () => void;
}) {
  return (
    <div className="flex flex-wrap gap-2">
      <Button
        variant="outline"
        size="sm"
        onClick={() => onDetect()}
        disabled={busy !== null}
      >
        {busy === "detect" ? (
          <Loader2 size={13} className="animate-spin" />
        ) : (
          <Search size={13} />
        )}
        Detectar novamente
      </Button>
      <Button
        variant="outline"
        size="sm"
        onClick={() => onRestart()}
        disabled={busy !== null}
      >
        {busy === "restart" ? (
          <Loader2 size={13} className="animate-spin" />
        ) : (
          <RefreshCw size={13} />
        )}
        Reiniciar motor
      </Button>
      <Button
        variant="ghost"
        size="sm"
        onClick={() => onRollback()}
        disabled={busy !== null}
      >
        {busy === "rollback" ? (
          <Loader2 size={13} className="animate-spin" />
        ) : (
          <Undo2 size={13} />
        )}
        Reverter OpenCode
      </Button>
      <Button
        variant="ghost"
        size="sm"
        onClick={() => onDiagnostics()}
        disabled={busy !== null}
      >
        {busy === "diagnostics" ? (
          <Loader2 size={13} className="animate-spin" />
        ) : (
          <Clipboard size={13} />
        )}
        Copiar diagnóstico
      </Button>
      <Button
        variant="ghost"
        size="sm"
        onClick={() => onRevealLogs()}
        disabled={!hasDiagnostics}
      >
        <Terminal size={13} />
        Abrir logs
      </Button>
    </div>
  );
}
