import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { seedWorkspace } from "./helpers";

test("file viewer handles binary, zip and large text files", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Files Viewers");
  writeFileSync(join(workspace.root, "binario.bin"), Buffer.from([0, 1, 2, 3, 0, 255]));
  writeFileSync(join(workspace.root, "pacote.zip"), Buffer.from("PK\x03\x04fake"));
  writeFileSync(join(workspace.root, "grande.txt"), "linha de teste\n".repeat(20000));

  await page.goto("/#/files");
  await page.getByText("binario.bin", { exact: true }).first().click();
  await expect(page.getByText("Este formato não tem prévia interna.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Abrir arquivo" })).toBeVisible();

  await page.getByText("pacote.zip", { exact: true }).first().click();
  await expect(page.getByRole("heading", { name: "pacote.zip" })).toBeVisible();

  await page.getByText("grande.txt", { exact: true }).first().click();
  await expect(page.getByText(/linha de teste/).first()).toBeVisible();
});
