import { apiGet } from "@/lib/api";
import type { AgentFile, ModelInfo, ModelHealth, StudioRuntime } from "@/lib/types";

interface CacheEntry<T> {
  value: T;
  at: number;
}

const TTL = 30_000;
const agents = new Map<string, CacheEntry<AgentFile[]>>();
let models: CacheEntry<ModelInfo[]> | null = null;
let modelHealth: CacheEntry<ModelHealth[]> | null = null;
const runtimes = new Map<string, CacheEntry<StudioRuntime>>();
const pending = new Map<string, Promise<unknown>>();

function fresh<T>(entry: CacheEntry<T> | null | undefined): entry is CacheEntry<T> {
  return Boolean(entry && Date.now() - entry.at < TTL);
}

function dedupe<T>(key: string, load: () => Promise<T>): Promise<T> {
  const existing = pending.get(key) as Promise<T> | undefined;
  if (existing) return existing;
  const promise = load().finally(() => pending.delete(key));
  pending.set(key, promise);
  return promise;
}

export function invalidateAgents() {
  agents.clear();
}

export function invalidateModels() {
  models = null;
  modelHealth = null;
}

export function invalidateRuntime() {
  runtimes.clear();
}

export async function loadAgents(workspaceId?: string | null): Promise<AgentFile[]> {
  const key = workspaceId ? `workspace:${workspaceId}` : "global";
  if (fresh(agents.get(key))) return agents.get(key)!.value;
  return dedupe(key, async () => {
    const requests: Array<Promise<AgentFile[]>> = [
      apiGet<AgentFile[]>("/api/studio/agents?scope=global").catch(() => []),
    ];
    if (workspaceId) {
      requests.unshift(
        apiGet<AgentFile[]>(
          `/api/studio/agents?scope=workspace&workspace_id=${encodeURIComponent(workspaceId)}`,
        ).catch(() => []),
      );
    }
    const lists = await Promise.all(requests);
    const seen = new Set<string>();
    const merged = lists
      .flat()
      .filter((agent): agent is AgentFile => Boolean(agent) && typeof agent.id === "string")
      .filter((agent) => {
      if (seen.has(agent.id)) return false;
      seen.add(agent.id);
      return true;
    });
    merged.sort((left, right) => left.id.localeCompare(right.id));
    agents.set(key, { value: merged, at: Date.now() });
    return merged;
  });
}

export async function loadModels(): Promise<ModelInfo[]> {
  if (fresh(models)) return models!.value;
  return dedupe("models", async () => {
    const response = await apiGet<{ data: ModelInfo[] }>("/api/models").catch(() => ({
      data: [],
    }));
    models = { value: response.data ?? [], at: Date.now() };
    return models.value;
  });
}

export async function loadModelHealth(): Promise<ModelHealth[]> {
  if (fresh(modelHealth)) return modelHealth!.value;
  return dedupe("model-health", async () => {
    const response = await apiGet<{ models: ModelHealth[] }>("/api/models/health").catch(
      () => ({ models: [] }),
    );
    modelHealth = { value: response.models ?? [], at: Date.now() };
    return modelHealth.value;
  });
}

export async function loadRuntime(
  workspaceId?: string | null,
): Promise<StudioRuntime | null> {
  if (!workspaceId) return null;
  const key = workspaceId;
  if (fresh(runtimes.get(key))) return runtimes.get(key)!.value;
  return dedupe(`runtime:${key}`, async () => {
    const value = await apiGet<StudioRuntime>(
      `/api/studio/runtime?workspace_id=${encodeURIComponent(workspaceId)}`,
    ).catch(() => null);
    if (value) runtimes.set(key, { value, at: Date.now() });
    return value;
  });
}
