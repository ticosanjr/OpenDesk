import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { seedWorkspace } from "./helpers";

const PNG_1X1 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

test("rich viewers handle json, csv, markdown, code and images", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Rich Viewers");
  writeFileSync(
    join(workspace.root, "dados.json"),
    JSON.stringify({ nome: "Ana", itens: [1, 2] }),
  );
  writeFileSync(
    join(workspace.root, "tabela.csv"),
    "nome,idade\nAna,30\nBia,25\n",
  );
  writeFileSync(
    join(workspace.root, "guia.md"),
    "# Guia\n\nIntro.\n\n## Instalação\n\nPassos.\n\n## Uso\n\nExemplo.\n\n### Detalhes\n\nFim.\n",
  );
  writeFileSync(
    join(workspace.root, "codigo.ts"),
    "const a = 1;\nconsole.log(a);\n",
  );
  writeFileSync(join(workspace.root, "foto.png"), Buffer.from(PNG_1X1, "base64"));

  await page.goto("/#/files");

  await page.getByText("dados.json", { exact: true }).first().click();
  await expect(page.getByRole("button", { name: "Árvore" })).toBeVisible();
  await page.getByLabel("Buscar no JSON").fill("Ana");
  await expect(page.getByText("1 resultado(s)")).toBeVisible();

  await page.getByText("tabela.csv", { exact: true }).first().click();
  await expect(page.getByRole("button", { name: "idade", exact: true })).toBeVisible();
  await page.getByLabel("Filtrar linhas da tabela").fill("Bia");
  await expect(page.getByText("1 de 2 linhas · 2 colunas")).toBeVisible();
  await expect(page.getByText("Bia", { exact: true })).toBeVisible();

  await page.getByText("guia.md", { exact: true }).first().click();
  const toc = page.getByRole("navigation", { name: "Sumário do documento" });
  await expect(toc).toBeVisible();
  await expect(toc.getByRole("button", { name: "Instalação" })).toBeVisible();
  await expect(
    page.getByRole("button", { name: /copiar markdown/i }),
  ).toBeVisible();

  await page.getByText("codigo.ts", { exact: true }).first().click();
  await page.getByLabel("Buscar no conteúdo").fill("console");
  await expect(page.getByText("1/1")).toBeVisible();

  await page.getByText("foto.png", { exact: true }).first().click();
  await page.getByRole("button", { name: "Aumentar zoom" }).click();
  await expect(page.locator("span").getByText("125%", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Ajustar" }).click();
  await expect(page.locator("span").getByText("100%", { exact: true })).toBeVisible();
});
