import { writeFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { seedWorkspace } from "./helpers";

function writeBundle(name: string, kind: string, items: unknown): string {
  const path = `/tmp/opencode/${name}`;
  writeFileSync(
    path,
    JSON.stringify({
      schema_version: 1,
      kind,
      exported_at: new Date().toISOString(),
      source_workspace: null,
      items,
    }),
  );
  return path;
}

test("personal context import validates the envelope and applies rules", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "Import Export UI");
  const wrong = writeBundle("wrong-kind.json", "structured_memory", {
    namespaces: [],
    notes: [],
    acls: {},
  });
  const valid = writeBundle("personal-context.json", "personal_context", {
    global: [],
    workspace: [
      {
        scope: "workspace",
        name: "Regra importada",
        instructions: "Regra importada pelo teste E2E",
        is_enabled: true,
      },
    ],
  });

  await page.goto("/#/learning");
  await page.getByRole("tab", { name: "Contexto Pessoal" }).click();
  await expect(page.getByText("Contexto pessoal", { exact: true })).toBeVisible();

  const importButton = page.getByRole("button", { name: "Importar JSON" }).first();
  let chooserPromise = page.waitForEvent("filechooser");
  await importButton.click();
  await (await chooserPromise).setFiles(wrong);
  await expect(page.getByText(/kind incompatível/)).toBeVisible();

  chooserPromise = page.waitForEvent("filechooser");
  await importButton.click();
  await (await chooserPromise).setFiles(valid);
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.getByRole("button", { name: "Mesclar" }).click();
  await expect(page.getByText("Regra importada pelo teste E2E")).toBeVisible();
});
