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
  RUST_LOG: "opendesk=info",
};

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: false,
  workers: 1,
  timeout: 60_000,
  expect: { timeout: 10_000 },
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:1420",
    channel: "chrome",
    headless: true,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  webServer: [
    {
      command:
        "bash -c 'rm -rf /tmp/opendesk-e2e && mkdir -p /tmp/opendesk-e2e/home /tmp/opendesk-e2e/xdg-config /tmp/opendesk-e2e/xdg-data /tmp/opendesk-e2e/xdg-state /tmp/opendesk-e2e/cache && exec cargo run --manifest-path src-tauri/Cargo.toml -- --headless'",
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
