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
          if (command === "controller_inputs") return ["E16 input"];
          if (command === "controller_connect") {
            window.controllerConnected = true;
            return;
          }
          if (command === "controller_disconnect") {
            window.controllerConnected = false;
            return;
          }
          if (command === "controller_reset") {
            window.controllerEdited = false;
            return;
          }
          if (command === "controller_status")
            return {
              connected: !!window.controllerConnected,
              received: 3,
              dropped: 0,
              error: null,
              view: { edited: !!window.controllerEdited, values: [] },
            };
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
              ...window.savedRouting,
            };
          }
          if (command === "select_composition" || command === "window_control")
            return;
          if (command === "save_settings") {
            window.savedRouting = args.routing;
            return;
          }
          if (command === "close_project") {
            loaded = false;
            playing = false;
            return;
          }
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
                  controls: window.liveControls || [],
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
                                touch: {
                                  offbeat_factor: 1.15,
                                  gap_factor: 1.25,
                                  velocity_jitter_factor: 0.95,
                                  requested_jitter_ticks: -4,
                                },
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
                            shared_accents: [
                              {
                                name: "drums",
                                amount: 0.7,
                                decision: t.accent,
                              },
                            ],
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
                                    envelope: { level: 0.6, impulses: 2 },
                                    automation: {
                                      segment: 2,
                                      cycle: 1,
                                      curve: "smooth",
                                      progress: 0.4,
                                    },
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
  await page.locator("#welcome-open").click();
  await expect(page.locator("#title")).toHaveText("Night maps");
  await expect(page.locator(".part-card")).toHaveCount(6);
  await page.locator("#settings-open").click();
  await page.locator("#destination").selectOption("@silent");
  await page.locator("#settings-save").click();
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
  await page.locator("#welcome-open").click();
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
  await page.locator("#welcome-new").click();
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
  await page.locator("#welcome-open").click();
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
  await page.locator("#projects-menu").click();
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
  await page.locator("#welcome-open").click();
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
  await page.locator("#welcome-open").click();
  await expect(page.locator("#destination")).toHaveValue("Phasecraft");
  await page.locator("#settings-open").click();
  await page.locator("#send-clock").check();
  await page.locator("#settings-save").click();
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
  await page.locator("#welcome-open").click();
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
  await page.locator("#welcome-open").click();
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
  await expect(page.locator("#detail-body")).toContainText(
    "segment 2, cycle 2 (smooth)",
  );
  await expect(page.locator("#detail-body")).toContainText(
    "Shared accent: drums",
  );
  await expect(page.locator("#detail-body")).toContainText(
    "envelope 0.600 from 2 impulses",
  );
});

test("project menu closes to home, routing persists, and inspector toggles", async ({
  page,
}) => {
  await boot(page);
  await page.locator("#welcome-open").click();
  await page.locator("#settings-open").click();
  await page.locator("#destination").selectOption("@silent");
  await page.locator("#send-clock").check();
  await page.locator("#settings-save").click();
  const hat = page.locator('[data-part="closed_hat"]');
  await hat.click();
  await expect(page.locator("#detail")).toBeVisible();
  await hat.click();
  await expect(page.locator("#detail")).toBeHidden();
  await hat.click();
  await page.locator("#detail-close").click();
  await expect(page.locator("#detail")).toBeHidden();
  await page.locator("#play").click();
  await page.locator("#stop").click();
  await expect(page.locator("#position")).toHaveText("1.1.1");
  await page.locator("#projects-menu").click();
  await page.locator("#close-project").click();
  await expect(page.locator("#welcome")).toBeVisible();
  await expect(page.locator("#system")).toBeHidden();
  await page.locator("#welcome-recent button").first().click();
  await page.locator("#settings-open").click();
  await expect(page.locator("#destination")).toHaveValue("@silent");
  await expect(page.locator("#send-clock")).toBeChecked();
});

test("titlebar controls invoke window actions and relationships appear on cards", async ({
  page,
}) => {
  await boot(page);
  await page.locator("#window-maximize").click();
  expect(
    await page.evaluate(() =>
      window.calls.some(
        (c) => c.command === "window_control" && c.args.action === "maximize",
      ),
    ),
  ).toBe(true);
  await page.locator("#welcome-open").click();
  await expect(
    page
      .locator(".card-formula")
      .filter({ hasText: /XOR|AND|NOT|OR/ })
      .first(),
  ).toBeVisible();
  await page.screenshot({
    path: "test-results/player-evolved.png",
    fullPage: true,
  });
});

test("settings dismissal cancels edits and only compositions scroll", async ({
  page,
}) => {
  await page.setViewportSize({ width: 900, height: 640 });
  await boot(page);
  await page.locator("#welcome-open").click();
  await page.locator("#settings-open").click();
  await page.locator("#destination").selectOption("@silent");
  await page.keyboard.press("Escape");
  await expect(page.locator("#destination")).toHaveValue("Phasecraft");
  await page.evaluate(() => {
    const nav = document.querySelector("#compositions");
    for (let i = 0; i < 60; i++) {
      const b = nav.firstElementChild.cloneNode(true);
      b.textContent = "Composition " + i;
      nav.append(b);
    }
  });
  expect(
    await page
      .locator(".sidebar")
      .evaluate((e) => e.scrollHeight <= e.clientHeight),
  ).toBe(true);
  expect(
    await page
      .locator("#compositions")
      .evaluate((e) => e.scrollHeight > e.clientHeight),
  ).toBe(true);
  await expect(page.locator("#projects-menu")).toBeInViewport();
  await expect(page.locator("#version")).toBeInViewport();
  await page.screenshot({
    path: "test-results/player-minimum.png",
    fullPage: true,
  });
});

test("controller connection is separate and temporary edits can be reset", async ({
  page,
}) => {
  await boot(page);
  await page.locator("#welcome-open").click();
  await page.locator("#settings-open").click();
  await page.locator("#controller-refresh").click();
  await expect(page.locator("#controller-input option")).toHaveCount(2);
  await page.locator("#controller-input").selectOption("E16 input");
  await page.locator("#controller-output").selectOption("Phasecraft");
  await page.locator("#controller-connect").click();
  await expect(page.locator("#controller-status")).toContainText("Connected");
  await expect(page.locator("#destination")).toHaveValue("Phasecraft");
  await page.evaluate(() => {
    window.controllerEdited = true;
  });
  await expect(
    page.getByText("Live Part edits", { exact: false }),
  ).toBeVisible();
  await page.locator("#controller-reset").click();
  await expect(
    page.getByText("Live Part edits", { exact: false }),
  ).toBeHidden();
  await page.locator("#controller-disconnect").click();
  await expect(page.locator("#controller-status")).toHaveText("Not connected.");
});

test("desired values show immediately while audible rings retain old structure", async ({
  page,
}) => {
  await boot(page);
  await page.locator("#welcome-open").click();
  const id = await page.locator(".part-card").first().getAttribute("data-part");
  await page.evaluate((id) => {
    window.liveControls = [
      {
        part: id,
        pending: true,
        values: [
          {
            parameter: "a_pulses",
            value: 7,
            applied: 4,
            enabled: true,
            pending: true,
          },
        ],
      },
    ];
  }, id);
  const card = page.locator(`.part-card[data-part="${id}"]`);
  await expect(card.locator(".pending-values")).toContainText("A pulses 4 → 7");
  await card.click();
  await expect(page.locator(".live-values")).toContainText(
    "A pulses: 7 · next bar (playing 4)",
  );
  await page.screenshot({
    path: "test-results/controller-pending.png",
    fullPage: true,
  });
  await page.evaluate(() => {
    window.liveControls[0].pending = false;
    window.liveControls[0].values[0].pending = false;
    window.liveControls[0].values[0].applied = 7;
  });
  await expect(card.locator(".pending-values")).toBeHidden();
  await expect(page.locator(".live-values")).toContainText("A pulses: 7");
  await expect(page.locator(".live-values")).not.toContainText("next bar");
});
