import { apiGet, apiPut } from "./api";

export interface FilePreferences {
  downloadDir: string;
  downloadAsk: boolean;
  htmlAllowScripts: boolean;
}

interface AppSetting {
  key: string;
  value_json: string;
}

function parse<T>(settings: AppSetting[], key: string, fallback: T): T {
  const entry = settings.find((item) => item.key === key);
  if (!entry) return fallback;
  try {
    return JSON.parse(entry.value_json) as T;
  } catch {
    return fallback;
  }
}

export async function loadFilePreferences(): Promise<FilePreferences> {
  try {
    const settings = await apiGet<AppSetting[]>("/api/settings");
    return {
      downloadDir: parse(settings, "download_dir", ""),
      downloadAsk: parse(settings, "download_ask", true),
      htmlAllowScripts: parse(settings, "html_allow_scripts", false),
    };
  } catch {
    return { downloadDir: "", downloadAsk: true, htmlAllowScripts: false };
  }
}

export async function saveFilePreferences(
  preferences: FilePreferences,
): Promise<void> {
  await Promise.all([
    apiPut("/api/settings/download_dir", preferences.downloadDir.trim()),
    apiPut("/api/settings/download_ask", preferences.downloadAsk),
    apiPut("/api/settings/html_allow_scripts", preferences.htmlAllowScripts),
  ]);
}
