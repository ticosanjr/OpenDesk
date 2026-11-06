import { HashRouter, Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "@/components/layout/AppShell";
import { DashboardPage } from "@/pages/DashboardPage";
import { KanbanPage } from "@/pages/KanbanPage";
import { ChatPage } from "@/pages/ChatPage";
import { ArtifactsPage } from "@/pages/ArtifactsPage";
import { FilesPage } from "@/pages/FilesPage";
import { RoutinesPage } from "@/pages/RoutinesPage";
import { StudioPage } from "@/pages/StudioPage";
import { RouterPage } from "@/pages/RouterPage";
import { IntegrationsPage } from "@/pages/IntegrationsPage";
import { SettingsPage } from "@/pages/SettingsPage";

export default function App() {
  return (
    <HashRouter>
      <Routes>
        <Route element={<AppShell />}>
          <Route index element={<DashboardPage />} />
          <Route path="kanban" element={<KanbanPage />} />
          <Route path="chat" element={<ChatPage />} />
          <Route path="artifacts" element={<ArtifactsPage />} />
          <Route path="files" element={<FilesPage />} />
          <Route path="routines" element={<RoutinesPage />} />
          <Route path="studio" element={<StudioPage />} />
          <Route path="router" element={<RouterPage />} />
          <Route path="integrations" element={<IntegrationsPage />} />
          <Route path="settings" element={<SettingsPage />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </HashRouter>
  );
}
