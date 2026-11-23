/**
 * Compatibility entry point. The runtime-aware implementation lives in
 * `runtime-env.ts` so browser/PWA builds never call Tauri invokes blindly.
 */
export { openWorkspacePath } from "./runtime-env";
