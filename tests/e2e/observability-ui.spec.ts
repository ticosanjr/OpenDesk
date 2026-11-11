import { expect, test } from "@playwright/test";
import { seedWorkspace } from "./helpers";

test("observability page exposes ranges and empty state", async ({ page, request }) => {
  await seedWorkspace(request, "Observability UI");
  await page.goto("/#/observability");
  await expect(page.getByRole("heading", { name: "Observabilidade" })).toBeVisible();
  await expect(page.getByText("Sem execuções no período")).toBeVisible();
  await page.getByRole("button", { name: "7d", exact: true }).click();
  await expect(page.getByRole("button", { name: "7d", exact: true })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
});
