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
                  traces: fixture.traces
                    .map((t) =>
                      window.testGroove && t.part === "closed_hat"
                        ? {
                            ...t,
                            event: {
                              tick: 2280,
                              duration_ticks: 100,
                              accent: { active: false, amount: 0 },
                              controls: [
                                {
                                  name: "filter",
                                  amount: 0.2,
                                  channel: 16,
                                  cc: 20,
                                  value: 25,
                                  reset: 25,
                                },
                              ],
                              groove: {
                                offset_ticks: 120,
                                requested_gate_ticks: 120,
                                ghost_roll: 0.2,
                                ghost: true,
                                run_before: 1,
                                run_after: 1,
                                velocity_factor: 0.4,
                              },
                            },
                          }
                        : t,
                    )
                    .map((t) =>
                      window.testParameters && t.part === "closed_hat"
                        ? {
                            ...t,
                            event: null,
                            parameters: [
                              {
                                name: "cutoff",
                                channel: 15,
                                cc: 75,
                                samples: [
                                  {
                                    tick: t.tick,
                                    base: 0.2,
                                    emphasis: 0,
                                    amount: 0.2,
                                    value: 25,
                                  },
                                  {
                                    tick: t.tick + 120,
                                    base: 0.7,
                                    emphasis: 0,
                                    amount: 0.7,
                                    value: 89,
                                  },
                                ],
                              },
                            ],
                          }
                        : t,
                    ),
                  progress: window.testProgress ?? fixture.progress,
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
  await page.locator("#open").click();
  await page.locator("#version").focus();
  await page.keyboard.press("Space");
  expect(
    await page.evaluate(() => window.calls.some((c) => c.command === "start")),
  ).toBe(false);
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

test("existing MIDI output supports explicit tempo and transport sync", async ({
  page,
}) => {
  await boot(page);
  await page.locator("#open").click();
  await expect(page.locator("#destination")).toHaveValue("Phasecraft");
  await page.locator("#send-clock").check();
  await page.locator("#play").click();
  await expect(page.locator("#send-clock")).toBeDisabled();
  expect(
    await page.evaluate(
      () => window.calls.findLast((c) => c.command === "start").args,
    ),
  ).toMatchObject({ port: "Phasecraft", sendClock: true });
});

test("groove inspection exposes timing and the hit flash waits for the onset", async ({
  page,
}) => {
  await boot(page);
  await page.evaluate(() => {
    window.testGroove = true;
    window.testProgress = 0.25;
  });
  await page.locator("#open").click();
  await page.locator("#play").click();
  const hat = page.locator('[data-part="closed_hat"]');
  await hat.click();
  await expect(page.locator("#detail-body")).toContainText("Timing +120 ticks");
  await expect(page.locator("#detail-body")).toContainText("ghost hit");
  await expect(page.locator("#detail-body")).toContainText(
    "channel 16 CC 20: 25",
  );
  await expect(page.locator("#detail-body")).toContainText(
    "reset 25 at note-off / Stop",
  );
  await expect(hat).not.toHaveClass(/fired/);
  await page.evaluate(() => (window.testProgress = 0.55));
  await expect(hat).toHaveClass(/fired/);
});

test("parameter inspector advances through a rest without flashing a note", async ({
  page,
}) => {
  await boot(page);
  await page.evaluate(() => {
    window.testParameters = true;
    window.testProgress = 0.25;
  });
  await page.locator("#open").click();
  await page.locator("#play").click();
  const hat = page.locator('[data-part="closed_hat"]');
  await hat.click();
  await expect(page.locator("#detail-body")).toContainText("base 0.200");
  await expect(page.locator("#detail-body")).toContainText("Resolved rest");
  await expect(hat).not.toHaveClass(/fired/);
  await page.evaluate(() => {
    window.testProgress = 0.75;
  });
  await expect(page.locator("#detail-body")).toContainText("base 0.700");
  await expect(page.locator("#detail-body")).toContainText(
    "channel 15 CC 75: 89",
  );
});
