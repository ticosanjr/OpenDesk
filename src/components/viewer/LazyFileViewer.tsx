import { lazy, Suspense } from "react";
import type { ComponentProps } from "react";

const FileViewer = lazy(() =>
  import("./FileViewer").then((module) => ({ default: module.FileViewer })),
);

export function LazyFileViewer(props: ComponentProps<typeof FileViewer>) {
  return (
    <Suspense
      fallback={
        <div className="flex min-h-0 flex-1 items-center justify-center p-6 text-[12px] text-zinc-500">
          Carregando visualizador…
        </div>
      }
    >
      <FileViewer {...props} />
    </Suspense>
  );
}
