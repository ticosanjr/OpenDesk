import { expect, test } from "@playwright/test";
import { api } from "./helpers";
import { seedWorkspace } from "./helpers";

test("chat transcript scrolls inside the card and stays above the composer", async ({
  page,
  request,
}) => {
  const workspace = await seedWorkspace(request, "Chat Scroll");
  const auth = api();
  const created = await request.post(`${auth.baseURL}/api/chat/threads`, {
    headers: auth.headers,
    data: {
      workspace_id: workspace.id,
      title: "Conversa longa",
      agent: "build",
      source: "desktop",
    },
  });
  expect(created.ok()).toBeTruthy();
  const thread = (await created.json()) as { id: string };

  for (const viewport of [
    { width: 1100, height: 700 },
    { width: 1440, height: 900 },
  ]) {
    await page.setViewportSize(viewport);
    await page.goto(`/#/chat?thread=${thread.id}`);
    await expect(page.getByTestId("chat-transcript")).toBeVisible();

    await page.evaluate(() => {
      const transcript = document.querySelector(
        '[data-testid="chat-transcript"]',
      ) as HTMLElement | null;
      if (!transcript) throw new Error("transcript missing");
      const filler = document.createElement("div");
      filler.style.height = "2400px";
      filler.textContent = "conteúdo longo";
      transcript.appendChild(filler);
    });

    const metrics = await page.evaluate(() => {
      const transcript = document.querySelector(
        '[data-testid="chat-transcript"]',
      ) as HTMLElement;
      const composer = document.querySelector(
        '[data-testid="chat-composer"]',
      ) as HTMLElement;
      const transcriptRect = transcript.getBoundingClientRect();
      const composerRect = composer.getBoundingClientRect();
      const before = transcript.scrollTop;
      transcript.scrollTop = transcript.scrollHeight;
      return {
        scrollHeight: transcript.scrollHeight,
        clientHeight: transcript.clientHeight,
        reachedBottom:
          transcript.scrollTop + transcript.clientHeight >=
          transcript.scrollHeight - 4,
        overlap: transcriptRect.bottom - composerRect.top,
        overflowY: window.getComputedStyle(transcript).overflowY,
        before,
      };
    });

    expect(metrics.scrollHeight).toBeGreaterThan(metrics.clientHeight + 100);
    expect(metrics.overflowY).toBe("auto");
    expect(metrics.reachedBottom).toBe(true);
    expect(metrics.overlap).toBeLessThanOrEqual(1);
  }

  await request.delete(`${auth.baseURL}/api/chat/threads/${thread.id}`, {
    headers: auth.headers,
  });
});
