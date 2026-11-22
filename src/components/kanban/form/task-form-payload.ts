export interface TaskPayloadInput {
  title: string;
  description: string;
  attachments: string[];
  tags: string;
  needsBrowser: boolean;
  browseAllowlist: string;
  executionMode: string;
  priority: number;
  agent: string;
  model: string | null;
  harness: string;
  effort: string;
  careLevel: "quick" | "standard" | "careful";
  email: string;
  exportToGdocs: boolean;
}

export function buildTaskPayload({
  title,
  description,
  attachments,
  tags,
  needsBrowser,
  browseAllowlist,
  executionMode,
  priority,
  agent,
  model,
  harness,
  effort,
  careLevel,
  email,
  exportToGdocs,
}: TaskPayloadInput) {
  const attachmentBlock =
    attachments.length > 0
      ? `\n\nArquivos anexados:\n${attachments
          .map((path) => `- ${path}`)
          .join("\n")}`
      : "";
  const parsedTags = tags
    .split(",")
    .map((tag) => tag.trim())
    .filter(
      (tag) =>
        Boolean(tag) &&
        tag !== "needs-browser" &&
        !tag.startsWith("browse-allowlist:"),
    );
  if (needsBrowser) {
    parsedTags.push("needs-browser");
    const allowlist = browseAllowlist
      .split(",")
      .map((domain) => domain.trim())
      .filter(Boolean)
      .join(",");
    if (allowlist) parsedTags.push(`browse-allowlist:${allowlist}`);
  }
  return {
    title: title.trim(),
    description: `${description.trim()}${attachmentBlock}`,
    execution_mode: executionMode,
    priority,
    assigned_agent: agent.trim() || null,
    preferred_model: model,
    preferred_harness: harness.trim() || null,
    effort_level: effort,
    care_level: careLevel,
    notify_email: email.trim() || null,
    export_to_gdocs: exportToGdocs,
    tags: parsedTags,
  };
}
