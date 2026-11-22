import { Archive, FileText } from "lucide-react";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import type { Artifact } from "@/lib/types";

export function PinnedArtifactsCard({
  artifacts,
  onOpenArtifact,
}: {
  artifacts: Artifact[];
  onOpenArtifact: (artifactId: string) => void;
}) {
  if (artifacts.length === 0) return null;
  return (
    <Card className="border-amber-500/20 bg-amber-500/[0.02]">
      <CardHeader className="py-3">
        <CardTitle className="flex items-center gap-2 text-[14px]">
          <Archive size={15} className="text-amber-300" />
          Entregáveis oficiais
        </CardTitle>
        <Badge variant="yellow">{artifacts.length} fixados</Badge>
      </CardHeader>
      <CardContent className="flex flex-wrap gap-2 pt-0">
        {artifacts.slice(0, 8).map((artifact) => (
          <button
            key={artifact.id}
            type="button"
            onClick={() => onOpenArtifact(artifact.id)}
            className="flex max-w-full items-center gap-1.5 rounded-lg border border-amber-500/20 bg-zinc-950/40 px-2.5 py-1.5 text-left text-[13px] text-zinc-300 hover:border-amber-500/40 hover:text-zinc-100"
            title={artifact.file_path}
          >
            <FileText size={12} className="shrink-0 text-amber-300" />
            <span className="max-w-56 truncate">{artifact.file_path}</span>
          </button>
        ))}
      </CardContent>
    </Card>
  );
}
