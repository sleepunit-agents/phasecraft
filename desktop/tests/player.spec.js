import { test, expect } from "@playwright/test";
import fs from "node:fs";
const fixture = JSON.parse(
  fs.readFileSync(new URL("./showcase.json", import.meta.url)),
);
async function boot(page) {
  await page.addInitScript(
    ({ fixture }) => {
      let loaded = false,
        playing = false;
      window.calls = [];
      window.__TAURI_INTERNALS__ = {
        invoke: async (command, args) => {
          window.calls.push({ command, args });
          if (command === "initial")
            return {
              recent: ["/music/night-maps"],
              version: { commit: "6d0bae765cf", platform: "windows-x64" },
            };
          if (command === "check_update") {
            if (window.updateOffline) throw new Error("offline");
            return { commit: window.remoteCommit || null, supported: true };
          }
          if (command === "install_update") {
            playing = false;
            if (window.updateFail)
              throw new Error("Signature verification failed");
            return new Promise(() => {}); // successful install exits the process
          }
          if (command === "destinations") return ["Phasecraft"];
          if (command === "plugin:dialog|open") return "/music/night-maps";
          if (command === "plugin:dialog|save") return "/music/new-set";
          if (command === "open_project" || command === "new_project") {
            loaded = true;
            playing = false;
            return {
              project: {
                path: "/music/night-maps",
                name: "Night maps",
                default: "/music/night-maps/compositions/showcase.toml",
                compositions: [
                  "/music/night-maps/compositions/showcase.toml",
                  "/music/night-maps/compositions/dnb.toml",
                ],
              },
              selected: "/music/night-maps/compositions/showcase.toml",
              port: "Phasecraft",
              virtual_port: false,
            };
          }
          if (command === "select_composition") return;
          if (command === "start") {
            playing = true;
            return;
          }
          if (command === "stop") {
            playing = false;
            return;
          }
          if (command === "snapshot")
            return loaded
              ? {
                  ...fixture,
                  playing,
                  step: playing ? 9 : null,
                  reload_error: window.reloadError || null,
                }
              : { playing: false, composition: null };
          throw new Error(command);
        },
      };
    },
    { fixture },
  );
  await page.goto("/");
}
test("open project, route, play, inspect and stop through the UI", async ({
  page,
}) => {
  await boot(page);
  await expect(page.locator("#play")).toBeDisabled();
  await page.locator("#open").click();
  await expect(page.locator("#title")).toHaveText("Night maps");
  await expect(page.locator(".part-card")).toHaveCount(6);
  await page.locator("#destination").selectOption("@silent");
  await page.locator("#play").click();
  await expect(page.locator("#state")).toContainText("SILENT PREVIEW");
  await expect(page.locator("#position")).toHaveText("1.3.2");
  await expect(page.locator("#destination")).toBeDisabled();
  await page.locator('[data-part="closed_hat"]').click();
  await expect(page.locator("#detail")).toBeVisible();
  await expect(page.locator("#detail-body")).toContainText("Admission");
  await page.screenshot({ path: "test-results/player.png", fullPage: true });
  await page.locator("#stop").click();
  await expect(page.locator("#state")).toContainText("STOPPED");
  await page.locator("#compositions button").last().click();
  expect(
    await page.evaluate(() =>
      window.calls.some((c) => c.command === "select_composition"),
    ),
  ).toBe(true);
});
test("reload errors are visible without stopping and recover when corrected", async ({
  page,
}) => {
  await boot(page);
  await page.locator("#open").click();
  await page.locator("#play").click();
  await page.evaluate(
    () => (window.reloadError = "trigger.probability must be within 0..1"),
  );
  await expect(page.locator("#reload-error")).toContainText(
    "Playing the last valid system",
  );
  await expect(page.locator("#state")).toContainText("PLAYING");
  await page.evaluate(() => (window.reloadError = null));
  await expect(page.locator("#reload-error")).toBeHidden();
});
test("new project and recent folders are accessible, including at minimum width", async ({
  page,
}) => {
  await page.setViewportSize({ width: 900, height: 700 });
  await boot(page);
  await page.locator("#new").click();
  await expect(page.locator(".part-card")).toHaveCount(6);
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  expect(
    await page.evaluate(() =>
      window.calls.some((c) => c.command === "new_project"),
    ),
  ).toBe(true);
});

test("update chip checks quietly, compares builds and installs only on click", async ({
  page,
}) => {
  await boot(page);
  await expect(page.locator("#update-chip")).toBeHidden();
  await page.evaluate(() => (window.remoteCommit = "b".repeat(40)));
  await page.locator("#version").click();
  await expect(page.locator("#update-chip")).toContainText("bbbbbbb");
  expect(
    await page.evaluate(() =>
      window.calls.some((c) => c.command === "install_update"),
    ),
  ).toBe(false);
  await page.locator("#open").click();
  await page.locator("#play").click();
  await page.locator("#update-chip").click();
  await expect(page.locator("#update-chip")).toBeDisabled();
  await expect(page.locator("#play")).toBeDisabled();
  await expect(page.locator("#state")).toContainText("STOPPED");
  expect(
    await page.evaluate(
      () => window.calls.filter((c) => c.command === "install_update").length,
    ),
  ).toBe(1);
});
test("offline checks and failed installs are recoverable", async ({ page }) => {
  await boot(page);
  await page.locator("#open").click();
  await page.locator("#play").click();
  await page.evaluate(() => (window.updateOffline = true));
  await page.locator("#version").click();
  await expect(page.locator("#update-chip")).toContainText(
    "Retry update check",
  );
  await expect(page.locator("#state")).toContainText("PLAYING");
  await page.evaluate(() => {
    window.updateOffline = false;
    window.remoteCommit = "c".repeat(40);
    window.updateFail = true;
  });
  await page.locator("#update-chip").click();
  await expect(page.locator("#update-chip")).toContainText("ccccccc");
  await page.locator("#update-chip").click();
  await expect(page.locator("#error")).toContainText(
    "Signature verification failed",
  );
  await expect(page.locator("#play")).toBeEnabled();
  await page.locator("#update-chip").click();
  await expect(page.locator("#update-chip")).toContainText("ccccccc");
});
