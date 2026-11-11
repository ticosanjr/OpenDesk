import { expect, test } from "@playwright/test";
import { mkdirSync, writeFileSync } from "node:fs";
import { seedWorkspace } from "./helpers";


const ROUTES = [
  "/",
  "/search",
  "/kanban",
  "/runs",
  "/chat",
  "/artifacts",
  "/files",
  "/routines",
  "/studio",
  "/router",
  "/integrations",
  "/settings",
];

test("every route scrolls to the end at desktop sizes", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Scroll");
  mkdirSync(`${workspace.root}/input`, { recursive: true });
  writeFileSync(
    `${workspace.root}/input/scroll.md`,
    `# Scroll\n\n${"linha de teste\\n".repeat(120)}`,
  );

  for (const viewport of [
    { width: 1100, height: 700 },
    { width: 1440, height: 900 },
  ]) {
    await page.setViewportSize(viewport);
    for (const route of ROUTES) {
      await page.goto(`/#${route}`);
      await page.waitForTimeout(450);

      const audit = (await page.evaluate(() => {
        const results: Array<{
          label: string;
          ok: boolean;
          scrollHeight: number;
          clientHeight: number;
          scrollTop: number;
        }> = [];

        const candidates = Array.from(
          document.querySelectorAll(
            "main, [class*='overflow-y-auto'], [class*='overflow-auto'], .scroll-area",
          ),
        ).filter((element) => {
          const style = window.getComputedStyle(element);
          return (
            ["auto", "scroll"].includes(style.overflowY) &&
            element.clientHeight > 0 &&
            element.scrollHeight > element.clientHeight + 4
          );
        });

        for (const element of candidates) {
          const before = element.scrollTop;
          element.scrollTop = element.scrollHeight;
          const ok =
            element.scrollTop + element.clientHeight >= element.scrollHeight - 4;
          results.push({
            label: `${element.tagName.toLowerCase()}.${String(
              (element as HTMLElement).className,
            ).slice(0, 80)}`,
            ok,
            scrollHeight: element.scrollHeight,
            clientHeight: element.clientHeight,
            scrollTop: element.scrollTop,
          });
          element.scrollTop = before;
        }

        const clipped = Array.from(document.querySelectorAll("*"))
          .filter((element) => {
            const style = window.getComputedStyle(element);
            return (
              style.overflowY === "hidden" &&
              element.clientHeight > 24 &&
              element.scrollHeight > element.clientHeight + 16 &&
              !element.closest("[aria-hidden='true']")
            );
          })
          .map((element) => ({
            label: `${element.tagName.toLowerCase()}.${String(
              (element as HTMLElement).className,
            ).slice(0, 80)}`,
            scrollHeight: element.scrollHeight,
            clientHeight: element.clientHeight,
          }));

        return { results, clipped };
      })) as {
        results: Array<{ label: string; ok: boolean }>;
        clipped: Array<{ label: string; scrollHeight: number; clientHeight: number }>;
      };

      const broken = audit.results.filter((item) => !item.ok);
      expect(
        broken,
        `${route} @ ${viewport.width}x${viewport.height}: scrollers that did not reach the end`,
      ).toEqual([]);
      expect(
        audit.clipped,
        `${route} @ ${viewport.width}x${viewport.height}: overflow-hidden content is clipped`,
      ).toEqual([]);
    }
  }

  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto(`/#/files?path=input/scroll.md`);
  await expect(page.getByText("linha de teste").first()).toBeVisible();
});
