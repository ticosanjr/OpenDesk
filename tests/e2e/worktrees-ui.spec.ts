import { execFileSync } from "node:child_process";
import { expect, test } from "@playwright/test";
import { api, seedWorkspace } from "./helpers";

function git(root: string, args: string[]) {
  return execFileSync("git", ["-C", root, ...args], { encoding: "utf8" });
}

test("worktree list shows a diff and merges after approval", async ({
  page,
  request,
}) => {
  try {
    execFileSync("git", ["--version"]);
  } catch {
    test.skip(true, "git indisponível");
  }
  const workspace = await seedWorkspace(request, "Worktrees");
  git(workspace.root, ["init"]);
  git(workspace.root, ["config", "user.email", "test@opendesk.local"]);
  git(workspace.root, ["config", "user.name", "Test"]);
  execFileSync("sh", [
    "-c",
    `printf 'base\\n' > '${workspace.root}/base.txt'`,
  ]);
  git(workspace.root, ["add", "-A"]);
  git(workspace.root, ["commit", "-m", "base"]);

  const auth = api();
  const prepared = await request.post(`${auth.baseURL}/api/worktrees`, {
    headers: auth.headers,
    data: { workspace_id: workspace.id },
  });
  expect(prepared.ok()).toBeTruthy();
  const worktree = (await prepared.json()) as { id: string; path: string };
  execFileSync("sh", [
    "-c",
    `printf 'isolado\\n' > '${worktree.path}/isolated.txt'`,
  ]);

  await page.goto("/#/worktrees");
  await expect(page.getByRole("heading", { name: "Execuções", level: 1 })).toBeVisible();
  await expect(page.getByText("isolated.txt", { exact: false })).toHaveCount(0);
  await page.getByRole("button", { name: "Diff", exact: true }).first().click();
  await expect(page.getByText("diff unificado")).toBeVisible();
  await expect(page.getByText("+isolado")).toBeVisible();
  await page.getByRole("dialog").getByLabel("Fechar").click();

  await page.getByRole("button", { name: "Mesclar", exact: true }).first().click();
  await expect(page.getByText("Mesclar worktree?")).toBeVisible();
  await page.getByRole("button", { name: "Mesclar", exact: true }).last().click();

  await expect
    .poll(() => {
      try {
        return execFileSync("sh", [
          "-c",
          `test -f '${workspace.root}/isolated.txt' && echo yes`,
        ])
          .toString()
          .trim();
      } catch {
        return "no";
      }
    })
    .toBe("yes");
});
