import { Activity, GitBranch } from "lucide-react";
import { SectionTabs } from "@/components/ui/section-tabs";

export function RunsPageTabs({
  value,
  onChange,
}: {
  value: "runs" | "worktrees";
  onChange: (id: string) => void;
}) {
  return (
    <SectionTabs
      value={value}
      onChange={onChange}
      items={[
        { id: "runs", label: "Execuções", icon: Activity },
        { id: "worktrees", label: "Worktrees", icon: GitBranch },
      ]}
    />
  );
}
