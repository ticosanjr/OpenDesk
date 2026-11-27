import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { ArtifactsTabContent } from "@/pages/artifacts/ArtifactsTabContent";

export function ArtifactsPage() {
  return (
    <PageBody>
      <PageHeader
        title="Artefatos"
        description="Quem criou, por quê, quando, diff e destinos."
      />
      <ArtifactsTabContent />
    </PageBody>
  );
}
