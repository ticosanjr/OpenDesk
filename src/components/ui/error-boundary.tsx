import * as React from "react";
import { AlertTriangle, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";

interface State {
  error: Error | null;
}

export class ErrorBoundary extends React.Component<
  { children: React.ReactNode; resetKey?: unknown },
  State
> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidUpdate(previous: { resetKey?: unknown }) {
    if (previous.resetKey !== this.props.resetKey && this.state.error) {
      this.setState({ error: null });
    }
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error("OpenDesk render error", error, info.componentStack);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="flex h-full items-center justify-center p-6">
        <div className="w-full max-w-lg rounded-xl border border-red-500/25 bg-red-500/[0.06] p-5">
          <div className="flex items-center gap-2 text-red-300">
            <AlertTriangle size={17} />
            <h2 className="text-sm font-semibold">Algo quebrou nesta tela</h2>
          </div>
          <p className="mt-2 whitespace-pre-wrap font-mono text-[10px] leading-relaxed text-red-200/80">
            {this.state.error.message}
          </p>
          <div className="mt-4 flex gap-2">
            <Button
              size="sm"
              variant="outline"
              onClick={() => this.setState({ error: null })}
            >
              <RefreshCw size={13} />
              Tentar novamente
            </Button>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => window.location.reload()}
            >
              Recarregar app
            </Button>
          </div>
        </div>
      </div>
    );
  }
}
