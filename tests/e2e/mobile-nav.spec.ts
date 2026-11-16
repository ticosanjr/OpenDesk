import { expect, test } from "@playwright/test";
import { seedWorkspace } from "./helpers";

test("mobile shell uses bottom navigation instead of the sidebar", async ({
  page,
  request,
}) => {
  await seedWorkspace(request, "Mobile");
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/#/");

  const nav = page.getByTestId("mobile-nav");
  await expect(nav).toBeVisible({ timeout: 30_000 });
  await expect(nav.getByRole("link", { name: "Caixa" })).toBeVisible();
  await expect(nav.getByRole("link", { name: "Chat" })).toBeVisible();
  await expect(nav.getByRole("link", { name: "Capturar" })).toBeVisible();
  await expect(nav.getByRole("link", { name: "Runs" })).toBeVisible();
  await expect(page.locator(".app-sidebar")).toBeHidden();

  await nav.getByRole("link", { name: "Chat" }).click();
  await expect(page).toHaveURL(/#\/chat$/);
  await expect(page.getByTestId("mobile-nav")).toBeVisible();

  // The main content reserves the height of the fixed bottom bar.
  const paddingBottom = await page
    .locator(".app-main")
    .evaluate((element) => getComputedStyle(element).paddingBottom);
  expect(Number.parseFloat(paddingBottom)).toBeGreaterThanOrEqual(64);
});

test("desktop keeps the sidebar and hides the mobile bar", async ({ page, request }) => {
  await seedWorkspace(request, "Desktop");
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/#/");
  await expect(page.getByTestId("mobile-nav")).toBeHidden();
  await expect(page.locator(".app-sidebar")).toBeVisible();
});

test("compact routes do not overflow the phone viewport", async ({ page, request }) => {
  await seedWorkspace(request, "Mobile Routes");
  await page.setViewportSize({ width: 390, height: 844 });
  for (const route of ["/", "/kanban", "/runs", "/chat", "/artifacts", "/settings"]) {
    await page.goto(`/#${route}`);
    await page.waitForTimeout(300);
    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth - window.innerWidth,
    );
    expect(overflow, `${route} overflowed horizontally`).toBeLessThanOrEqual(1);
  }
});

