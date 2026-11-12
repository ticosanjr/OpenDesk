import { useState } from "react";
import { Download, RefreshCw, Trash2, TriangleAlert } from "lucide-react";
import { Link } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { CountPill } from "@/components/ui/count-pill";
import { IconLabel } from "@/components/ui/icon-label";
import { LoadingState } from "@/components/ui/loading-state";
import { Modal } from "@/components/ui/modal";
import type { InstallPlan, PackItem, PackSummary } from "./pack-types";

const ITEM_KINDS = [
  { id: "agent", label: "Agentes" },
  { id: "skill", label: "Skills" },
  { id: "command", label: "Comandos" },
  { id: "recipe", label: "Receitas" },
] as const;

export function PackInstallModal({
  selected,
  plan,
  planLoading,
  installing,
  removing,
  overwritePaths,
  activeWorkspaceId,
  items,
  itemsLoading,
  installingItem,
  onClose,
  onToggleOverwrite,
  onInstall,
  onRemove,
  onInstallItem,
}: {
  selected: PackSummary | null;
  plan: InstallPlan | null;
  planLoading: boolean;
  installing: boolean;
  removing: boolean;
  overwritePaths: Set<string>;
  activeWorkspaceId: string | null;
  items: PackItem[];
  itemsLoading: boolean;
  installingItem: string | null;
  onClose: () => void;
  onToggleOverwrite: (path: string, checked: boolean) => void;
  onInstall: () => void;
  onRemove: () => void;
  onInstallItem: (kind: string, id: string) => void;
}) {
  const [itemKind, setItemKind] = useState<(typeof ITEM_KINDS)[number]["id"]>("agent");
  const [itemStatus, setItemStatus] = useState<"all" | "installed" | "available">("all");
  const visibleItems = items.filter(
    (item) =>
      item.kind === itemKind &&
      (itemStatus === "all" ||
        (itemStatus === "installed" ? item.installed : !item.installed)),
  );

  return (
    <Modal
      open={selected !== null}
      onClose={onClose}
      title={selected?.name ?? "Pacote"}
      description={selected?.description}
      size="lg"
    >
      {selected ? (
        <div className="space-y-3">
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant="default">{selected.counts.agents} agentes</Badge>
            <Badge variant="default">{selected.counts.skills} skills</Badge>
            <Badge variant="default">{selected.counts.commands} comandos</Badge>
            <Badge variant="default">{selected.counts.recipes} receitas</Badge>
            <Badge variant="default">{selected.counts.routines} rotinas</Badge>
            {selected.installed ? (
              <Badge
                variant={
                  selected.health.status === "partial"
                    ? "red"
                    : selected.update_available
                      ? "yellow"
                      : selected.health.status === "modified"
                        ? "purple"
                        : "green"
                }
              >
                {selected.health.status === "partial"
                  ? "parcial"
                  : selected.update_available
                    ? "atualização"
                    : selected.health.status === "modified"
                      ? "personalizado"
                      : "instalado"}
              </Badge>
            ) : null}
          </div>
          {selected.installed && selected.update_available ? (
            <p className="text-[10px] text-amber-200">
              Versão instalada {selected.installed_version ?? "?"}; disponível{" "}
              {selected.version}.
            </p>
          ) : null}
          {selected.health.status === "partial" ? (
            <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[10px] text-red-200">
              Faltam {selected.health.missing_files} arquivo(s) e{" "}
              {selected.health.missing_routines} rotina(s). Use “Reparar” para
              recriar o que falta; arquivos personalizados aparecem como conflito
              e exigem confirmação.
            </p>
          ) : null}

          {selected.requires.tools.length || selected.requires.integrations.length ? (
            <div className="rounded-lg border border-amber-500/20 bg-amber-500/[0.05] px-3 py-2 text-[10px] text-amber-200">
              <strong>Requisitos:</strong>{" "}
              {[...selected.requires.tools, ...selected.requires.integrations].join(", ")}
            </div>
          ) : null}

          <div className="rounded-xl border border-zinc-800/70 bg-zinc-950/30">
            <div className="flex flex-wrap gap-1 border-b border-zinc-800/70 p-1.5">
              {ITEM_KINDS.map((kind) => {
                const count = items.filter((item) => item.kind === kind.id).length;
                return (
                  <button
                    key={kind.id}
                    type="button"
                    aria-pressed={itemKind === kind.id}
                    onClick={() => setItemKind(kind.id)}
                    className={
                      itemKind === kind.id
                        ? "rounded-md bg-zinc-800 px-2 py-1 text-[10px] text-zinc-100"
                        : "rounded-md px-2 py-1 text-[10px] text-zinc-500 hover:text-zinc-200"
                    }
                  >
                    {kind.label} <span className="font-mono text-zinc-500">{count}</span>
                  </button>
                );
              })}
            </div>
            <div className="flex flex-wrap items-center gap-1 border-t border-zinc-800/60 px-2 py-1.5">
              {([
                ["all", "Todos"],
                ["installed", "Instalados"],
                ["available", "Disponíveis"],
              ] as const).map(([value, label]) => (
                <button
                  key={value}
                  type="button"
                  aria-pressed={itemStatus === value}
                  onClick={() => setItemStatus(value)}
                  className={
                    itemStatus === value
                      ? "rounded-md bg-zinc-800 px-2 py-1 text-[9px] text-zinc-100"
                      : "rounded-md px-2 py-1 text-[9px] text-zinc-500 hover:text-zinc-200"
                  }
                >
                  {label}
                </button>
              ))}
            </div>
            <div className="max-h-56 overflow-y-auto p-1.5">
              {itemsLoading ? (
                <LoadingState compact label="Carregando itens…" />
              ) : visibleItems.length === 0 ? (
                <p className="p-2 text-[10px] text-zinc-500">Nenhum item neste tipo.</p>
              ) : (
                visibleItems.map((item) => (
                  <details
                    key={`${item.kind}:${item.id}`}
                    className="rounded-lg border border-zinc-800/60 bg-zinc-950/40 p-2 [&_summary::-webkit-details-marker]:hidden"
                  >
                    <summary className="flex cursor-pointer items-center gap-2 text-[10px] text-zinc-300">
                      <span className="min-w-0 flex-1 truncate">{item.name}</span>
                      <Badge variant="default">{item.kind}</Badge>
                      <Badge
                        variant={
                          item.status === "installed"
                            ? "green"
                            : item.status === "modified"
                              ? "yellow"
                              : "default"
                        }
                        title={
                          item.installed
                            ? "Todos os arquivos deste item estão instalados no workspace"
                            : item.status === "modified"
                              ? "O item existe, mas foi modificado"
                              : "Este item ainda não está instalado"
                        }
                      >
                        {item.status === "installed"
                          ? "instalado"
                          : item.status === "modified"
                            ? "modificado"
                            : "disponível"}
                      </Badge>
                      <Button
                        variant="outline"
                        size="sm"
                        className="h-6 px-2 text-[9px]"
                        disabled={!activeWorkspaceId || installingItem === `${item.kind}:${item.id}`}
                        onClick={(event) => {
                          event.preventDefault();
                          event.stopPropagation();
                          onInstallItem(item.kind, item.id);
                        }}
                      >
                        {installingItem === `${item.kind}:${item.id}`
                          ? "Instalando…"
                          : item.installed
                            ? "Reaplicar"
                            : "Instalar"}
                      </Button>
                    </summary>
                    {item.description ? (
                      <p className="mt-1 text-[9px] leading-snug text-zinc-500">
                        {item.description}
                      </p>
                    ) : null}
                    <pre className="mt-1.5 max-h-40 overflow-auto whitespace-pre-wrap rounded border border-zinc-800/70 bg-black/25 p-2 font-mono text-[9px] leading-relaxed text-zinc-400">
                      {item.content}
                    </pre>
                    {item.files.length > 0 ? (
                      <p className="mt-1 font-mono text-[8px] text-zinc-500">
                        Arquivos: {item.files.join(", ")}
                      </p>
                    ) : null}
                  </details>
                ))
              )}
            </div>
          </div>

          {!activeWorkspaceId ? (
            <p className="rounded-lg border border-zinc-800 px-3 py-2 text-[10px] text-zinc-500">
              Selecione um workspace ativo para adicionar este pacote.
            </p>
          ) : planLoading ? (
            <LoadingState compact label="Calculando arquivos e conflitos…" />
          ) : plan ? (
            <>
              <div className="grid grid-cols-3 gap-2 text-center">
                <div className="rounded-lg border border-zinc-800 px-2 py-1.5">
                  <CountPill
                    value={plan.files.filter((file) => file.status === "create").length}
                    tone="green"
                  />
                  <p className="mt-1 text-[9px] text-zinc-500">novos</p>
                </div>
                <div className="rounded-lg border border-zinc-800 px-2 py-1.5">
                  <CountPill
                    value={plan.files.filter((file) => file.status === "identical").length}
                  />
                  <p className="mt-1 text-[9px] text-zinc-500">iguais</p>
                </div>
                <div className="rounded-lg border border-zinc-800 px-2 py-1.5">
                  <CountPill
                    value={plan.conflicts}
                    tone={plan.conflicts ? "red" : "default"}
                  />
                  <p className="mt-1 text-[9px] text-zinc-500">conflitos</p>
                </div>
              </div>
              {plan.conflicts > 0 ? (
                <>
                  <p className="flex items-start gap-1.5 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[10px] text-red-200">
                    <TriangleAlert size={12} className="mt-0.5 shrink-0" />
                    Marque os arquivos que podem ser substituídos. Os não marcados
                    serão mantidos como estão; substituições fazem backup.
                  </p>
                  <div className="max-h-32 space-y-1 overflow-y-auto rounded-lg border border-zinc-800/70 p-2">
                    {plan.files
                      .filter((file) => file.status === "conflict")
                      .map((file) => (
                        <label
                          key={file.path}
                          className="flex items-center gap-2 text-[10px] text-zinc-300"
                        >
                          <input
                            type="checkbox"
                            checked={overwritePaths.has(file.path)}
                            onChange={(event) =>
                              onToggleOverwrite(file.path, event.target.checked)
                            }
                            className="accent-blue-500"
                          />
                          <span className="truncate font-mono text-[9px]">
                            {file.path}
                          </span>
                        </label>
                      ))}
                  </div>
                </>
              ) : null}
              {plan.routines.length > 0 ? (
                <p className="text-[10px] text-zinc-500">
                  {plan.routines.length} rotina(s) serão criadas <strong>inativas</strong>.
                </p>
              ) : null}
              <div className="max-h-40 overflow-y-auto rounded-lg border border-zinc-800/70">
                {plan.files.slice(0, 80).map((file) => (
                  <div
                    key={file.path}
                    className="flex items-center gap-2 border-b border-zinc-800/50 px-2.5 py-1 last:border-0"
                  >
                    <span className="min-w-0 flex-1 truncate font-mono text-[9px] text-zinc-400">
                      {file.path}
                    </span>
                    <Badge
                      variant={
                        file.status === "conflict"
                          ? "red"
                          : file.status === "identical"
                            ? "default"
                            : "green"
                      }
                    >
                      {file.status}
                    </Badge>
                  </div>
                ))}
              </div>
            </>
          ) : null}

          <div className="flex flex-wrap justify-end gap-2 border-t border-zinc-800/70 pt-3">
            {selected.installed ? (
              <Button
                variant="outline"
                size="sm"
                className="border-red-500/30 text-red-300"
                onClick={onRemove}
                disabled={removing}
              >
                <IconLabel icon={<Trash2 size={13} />}>Remover</IconLabel>
              </Button>
            ) : null}
            <Button
              size="sm"
              onClick={onInstall}
              disabled={!activeWorkspaceId || installing || !plan}
            >
              <IconLabel
                icon={
                  installing ? (
                    <RefreshCw size={13} className="animate-spin" />
                  ) : (
                    <Download size={13} />
                  )
                }
              >
                {selected.health.status === "partial"
                  ? "Reparar"
                  : selected.update_available
                    ? "Atualizar"
                    : selected.installed
                      ? "Reinstalar"
                      : "Adicionar ao workspace"}
              </IconLabel>
            </Button>
          </div>
          {selected.installed ? (
            <div className="flex flex-wrap gap-2 text-[10px] text-zinc-500">
              <Link to="/studio" className="hover:text-zinc-200">
                ver agentes
              </Link>
              <Link to="/recipes" className="hover:text-zinc-200">
                ver receitas
              </Link>
              <Link to="/routines" className="hover:text-zinc-200">
                ver rotinas
              </Link>
            </div>
          ) : null}
        </div>
      ) : null}
    </Modal>
  );
}
