import { Loader2, Plus, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import {
  CHANNELS,
  PROFILES,
  TRUST,
  inputClass,
  type ChannelRoute,
} from "./types";

export interface RouteForm {
  channel: string;
  identity: string;
  agent: string;
  trust: string;
  profile: string;
}

export function RoutesSection({
  routes,
  loading,
  busy,
  form,
  onFormChange,
  onAdd,
  onRemove,
}: {
  routes: ChannelRoute[];
  loading: boolean;
  busy: string | null;
  form: RouteForm;
  onFormChange: (next: RouteForm) => void;
  onAdd: () => void;
  onRemove: (id: string) => void;
}) {
  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-end gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2">
        <select
          value={form.channel}
          onChange={(event) => onFormChange({ ...form, channel: event.target.value })}
          className={inputClass}
        >
          {CHANNELS.map((channel) => (
            <option key={channel} value={channel}>
              {channel}
            </option>
          ))}
        </select>
        <input
          value={form.identity}
          onChange={(event) => onFormChange({ ...form, identity: event.target.value })}
          placeholder="identidade ou *"
          className={`${inputClass} w-40 font-mono`}
        />
        <input
          value={form.agent}
          onChange={(event) => onFormChange({ ...form, agent: event.target.value })}
          placeholder="agente"
          className={`${inputClass} w-28`}
        />
        <select
          value={form.trust}
          onChange={(event) => onFormChange({ ...form, trust: event.target.value })}
          className={inputClass}
        >
          {TRUST.map((trust) => (
            <option key={trust} value={trust}>
              {trust}
            </option>
          ))}
        </select>
        <select
          value={form.profile}
          onChange={(event) => onFormChange({ ...form, profile: event.target.value })}
          className={inputClass}
        >
          {PROFILES.map((profile) => (
            <option key={profile} value={profile}>
              {profile}
            </option>
          ))}
        </select>
        <Button
          size="sm"
          className="h-8"
          disabled={busy === "route"}
          onClick={onAdd}
        >
          <Plus size={12} />
          Rota
        </Button>
      </div>

      {loading ? (
        <div className="flex items-center gap-2 py-3 text-[12.5px] text-zinc-500">
          <Loader2 size={13} className="animate-spin" />
          carregando rotas…
        </div>
      ) : routes.length === 0 ? (
        <EmptyState
          title="Nenhuma rota"
          description="Sem rota, mensagens recebidas são recusadas. Crie ao menos uma rota por canal."
        />
      ) : (
        <div className="overflow-hidden rounded-lg border border-zinc-800/70">
          <table className="w-full text-left text-[12.5px]">
            <thead className="bg-zinc-900/60 text-[11px] uppercase tracking-[0.08em] text-zinc-500">
              <tr>
                <th className="px-3 py-1.5">canal</th>
                <th className="px-3 py-1.5">identidade</th>
                <th className="px-3 py-1.5">agente</th>
                <th className="px-3 py-1.5">trust</th>
                <th className="px-3 py-1.5">perfil</th>
                <th className="px-3 py-1.5" />
              </tr>
            </thead>
            <tbody>
              {routes.map((route) => (
                <tr key={route.id} className="border-t border-zinc-800/60">
                  <td className="px-3 py-1.5 font-mono text-zinc-300">
                    {route.channel}
                  </td>
                  <td className="px-3 py-1.5 font-mono text-zinc-400">
                    {route.identity}
                  </td>
                  <td className="px-3 py-1.5 text-zinc-300">{route.agent}</td>
                  <td className="px-3 py-1.5">
                    <Badge
                      variant={
                        route.trust_level === "trusted"
                          ? "green"
                          : route.trust_level === "internal"
                            ? "blue"
                            : "default"
                      }
                    >
                      {route.trust_level}
                    </Badge>
                  </td>
                  <td className="px-3 py-1.5 text-zinc-400">
                    {route.permission_profile}
                  </td>
                  <td className="px-3 py-1.5 text-right">
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-6 px-2"
                      disabled={busy === `route:${route.id}`}
                      onClick={() => onRemove(route.id)}
                    >
                      <Trash2 size={11} />
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
