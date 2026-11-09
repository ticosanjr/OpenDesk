import { lazy } from "react";
import { HashRouter, Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "@/components/layout/AppShell";

const DashboardPage = lazy(() =>
  import("@/pages/DashboardPage").then((module) => ({ default: module.DashboardPage })),
);
const KanbanPage = lazy(() =>
  import("@/pages/KanbanPage").then((module) => ({ default: module.KanbanPage })),
);
const SearchPage = lazy(() =>
  import("@/pages/SearchPage").then((module) => ({ default: module.SearchPage })),
);
const CalendarPage = lazy(() =>
  import("@/pages/CalendarPage").then((module) => ({ default: module.CalendarPage })),
);
const RunsPage = lazy(() =>
  import("@/pages/RunsPage").then((module) => ({ default: module.RunsPage })),
);
const RunDetailPage = lazy(() =>
  import("@/pages/RunDetailPage").then((module) => ({ default: module.RunDetailPage })),
);
const ChatPage = lazy(() =>
  import("@/pages/ChatPage").then((module) => ({ default: module.ChatPage })),
);
const ArtifactsPage = lazy(() =>
  import("@/pages/ArtifactsPage").then((module) => ({ default: module.ArtifactsPage })),
);
const FilesPage = lazy(() =>
  import("@/pages/FilesPage").then((module) => ({ default: module.FilesPage })),
);
const RoutinesPage = lazy(() =>
  import("@/pages/RoutinesPage").then((module) => ({ default: module.RoutinesPage })),
);
const PacksPage = lazy(() =>
  import("@/pages/PacksPage").then((module) => ({ default: module.PacksPage })),
);

const StudioPage = lazy(() =>
  import("@/pages/StudioPage").then((module) => ({ default: module.StudioPage })),
);
const LearningPage = lazy(() =>
  import("@/pages/LearningPage").then((module) => ({ default: module.LearningPage })),
);
const GovernancePage = lazy(() =>
  import("@/pages/GovernancePage").then((module) => ({ default: module.GovernancePage })),
);
const RouterPage = lazy(() =>
  import("@/pages/RouterPage").then((module) => ({ default: module.RouterPage })),
);
const IntegrationsPage = lazy(() =>
  import("@/pages/IntegrationsPage").then((module) => ({ default: module.IntegrationsPage })),
);
const RecipesPage = lazy(() =>
  import("@/pages/RecipesPage").then((module) => ({ default: module.RecipesPage })),
);
const SettingsPage = lazy(() =>
  import("@/pages/SettingsPage").then((module) => ({ default: module.SettingsPage })),
);
const CapturePage = lazy(() =>
  import("@/pages/CapturePage").then((module) => ({ default: module.CapturePage })),
);

export default function App() {
  return (
    <HashRouter>
      <Routes>
        <Route path="capture" element={<CapturePage />} />
        <Route element={<AppShell />}>
          <Route index element={<DashboardPage />} />
          <Route path="search" element={<SearchPage />} />
          <Route path="kanban" element={<KanbanPage />} />
          <Route path="calendar" element={<CalendarPage />} />
          <Route path="runs" element={<RunsPage />} />
          <Route path="runs/:runId" element={<RunDetailPage />} />
          <Route path="chat" element={<ChatPage />} />
          <Route path="artifacts" element={<ArtifactsPage />} />
          <Route path="files" element={<FilesPage />} />
          <Route path="routines" element={<RoutinesPage />} />
          <Route path="recipes" element={<RecipesPage />} />
          <Route path="packs" element={<PacksPage />} />
          <Route path="studio" element={<StudioPage />} />
          <Route path="learning" element={<LearningPage />} />
          <Route path="governance" element={<GovernancePage />} />
          <Route path="router" element={<RouterPage />} />
          <Route path="integrations" element={<IntegrationsPage />} />
          <Route path="settings" element={<SettingsPage />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </HashRouter>
  );
}
