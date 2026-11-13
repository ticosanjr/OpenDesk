import { defineConfig } from "@playwright/test";

const e2eDir = "/tmp/opendesk-e2e";

const e2eEnv = {
  ...process.env,
  OPENDESK_DATA_DIR: `${e2eDir}/data`,
  OPENDESK_CONFIG_DIR: `${e2eDir}/app-config`,
  OPENDESK_CACHE_DIR: `${e2eDir}/cache`,
  XDG_CONFIG_HOME: `${e2eDir}/xdg-config`,
  XDG_DATA_HOME: `${e2eDir}/xdg-data`,
  XDG_STATE_HOME: `${e2eDir}/xdg-state`,
  XDG_CACHE_HOME: `${e2eDir}/cache`,
  OPENDESK_API_PORT: "4317",
  OPENDESK_SKIP_DAEMON: "1",
  OPENDESK_E2E: "1",
  OPENDESK_RATE_LIMIT_READ: "100000",
  OPENDESK_RATE_LIMIT_WRITE: "100000",
  OPENDESK_RATE_LIMIT_SENSITIVE: "100000",
  OPENDESK_ALLOW_TEMP_WORKSPACES: "1",
  OPENDESK_WORKSPACES_ROOT: `${e2eDir}/workspaces-root`,
  RUST_LOG: "opendesk=info",
};

export default defineConfig({
  testDir: "./tests/visual",
  fullyParallel: false,
  workers: 1,
  timeout: 180_000,
  expect: {
    timeout: 15_000,
    toHaveScreenshot: {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.015,
    },
  },
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:1420",
    headless: true,
    viewport: { width: 1440, height: 900 },
    timezoneId: "America/Sao_Paulo",
    trace: "off",
    screenshot: "off",
  },
  webServer: [
    {
      command:
        "bash -c 'bash scripts/e2e-seed.sh && exec cargo run --manifest-path src-tauri/Cargo.toml -- --headless'",
      url: "http://127.0.0.1:4317/healthz",
      timeout: 240_000,
      reuseExistingServer: false,
      env: e2eEnv,
      stdout: "pipe",
      stderr: "pipe",
    },
    {
      command: "node scripts/e2e-vite.mjs",
      url: "http://localhost:1420",
      timeout: 60_000,
      reuseExistingServer: false,
      env: e2eEnv,
      stdout: "pipe",
      stderr: "pipe",
    },
  ],
});
