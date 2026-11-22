import { RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";

export function RunLogErrorState({
  error,
  refreshing,
  onRetry,
}: {
  error: string;
  refreshing: boolean;
  onRetry: () => void;
}) {
  return (
    <div className="space-y-3 p-4">
      <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
        {error}
      </p>
      <Button
        size="sm"
        variant="outline"
        onClick={onRetry}
        disabled={refreshing}
      >
        <RefreshCw
          size={13}
          className={refreshing ? "animate-spin" : undefined}
        />
        Tentar novamente
      </Button>
    </div>
  );
}
