import { readFileSync } from "node:fs";
import { spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

const dataDir = process.env.OPENDESK_DATA_DIR ?? "/tmp/opendesk-e2e/data";

let endpoint;
for (let attempt = 0; attempt < 160; attempt += 1) {
  try {
    endpoint = JSON.parse(readFileSync(`${dataDir}/api.json`, "utf8"));
    break;
  } catch {
    await sleep(250);
  }
}

if (!endpoint) {
  console.error(`OpenDesk e2e: api.json not found in ${dataDir}`);
  process.exit(1);
}

const child = spawn("npm", ["run", "dev", "--", "--port", "1420"], {
  stdio: "inherit",
  env: {
    ...process.env,
    VITE_OPENDESK_PORT: String(endpoint.port),
    VITE_OPENDESK_TOKEN: endpoint.token,
  },
});

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => child.kill(signal));
}

child.on("exit", (code) => process.exit(code ?? 0));
