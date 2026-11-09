export interface BrowserStatus {
  binary?: string | null;
  configured: boolean;
  skill: boolean;
  scope: string;
  config_path?: string | null;
  install_hint: string;
  command: string[];
}
