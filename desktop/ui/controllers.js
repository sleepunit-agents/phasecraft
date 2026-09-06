export function setupController(invoke) {
  const settings = document.getElementById("settings-dialog");
  const box = document.createElement("section");
  box.className = "controller-settings";
  box.innerHTML = `<h3>Controller · E16</h3><p class="muted">Connect the E16 separately from the MIDI destination used by Ableton.</p><label for="controller-input">INPUT FROM CONTROLLER</label><select id="controller-input"><option value="">Choose input…</option></select><label for="controller-output">FEEDBACK TO CONTROLLER</label><select id="controller-output"><option value="">Choose output…</option></select><div class="project-actions"><button id="controller-refresh">Refresh ports</button><button id="controller-connect">Connect</button><button id="controller-disconnect">Disconnect</button><button id="controller-reset">Reset live edits</button></div><p id="controller-status" role="status">Not connected.</p><p class="muted">Loop compositions, up to 32 Parts. Edits apply at the next bar; Stop, Reset, or a valid file reload restores the score. Connections are selected each app launch.</p><details><summary>Selected Part values</summary><div id="controller-values"></div></details>`;
  settings.append(box);
  const notice = document.createElement("div");
  notice.className = "notice";
  notice.hidden = true;
  notice.textContent =
    "Live Part edits · next-bar changes · Stop or Reset restores the score. Not saved to TOML.";
  document.getElementById("system").before(notice);
  const el = (id) => document.getElementById(`controller-${id}`);
  let busy = false;
  let actionError = null;
  async function action(fn) {
    if (busy) return;
    busy = true;
    actionError = null;
    try {
      await fn();
      await poll();
    } catch (e) {
      actionError = String(e);
      el("status").textContent = actionError;
    } finally {
      busy = false;
    }
  }
  function options(select, names) {
    const chosen = select.value;
    select.replaceChildren(new Option("Choose port…", ""));
    for (const name of names) select.add(new Option(name, name));
    select.value = chosen;
  }
  el("refresh").onclick = () =>
    action(async () => {
      const [inputs, outputs] = await Promise.all([
        invoke("controller_inputs"),
        invoke("destinations"),
      ]);
      options(el("input"), inputs);
      options(el("output"), outputs);
    });
  el("connect").onclick = () =>
    action(async () => {
      if (!el("input").value || !el("output").value)
        throw new Error("Choose both controller ports first.");
      await invoke("controller_connect", {
        input: el("input").value,
        output: el("output").value,
      });
    });
  el("disconnect").onclick = () =>
    action(() => invoke("controller_disconnect"));
  el("reset").onclick = () => action(() => invoke("controller_reset"));
  async function poll() {
    const s = await invoke("controller_status");
    if (!s) return;
    notice.hidden = !(s.parts || [s.view]).some((v) => v?.edited || v?.pending);
    if (!settings.open) return;
    el("status").textContent =
      actionError ||
      s.error ||
      (s.connected
        ? s.received
          ? `Connected · ${s.received} messages · ${s.dropped} dropped`
          : "Ports open · waiting for E16"
        : "Not connected.");
    el("disconnect").disabled = !s.connected;
    el("values").replaceChildren(
      ...(s.view?.values || []).map((v) => {
        const row = document.createElement("p");
        row.textContent = `${v.label}${v.pending ? " · NEXT BAR" : ""}: ${v.enabled ? (["level", "cutoff", "decay", "trigger_probability", "accent_probability", "accent_amount"].includes(v.parameter) ? Math.round(v.value * 100) + "%" : v.value) : "unavailable"}`;
        return row;
      }),
    );
  }
  setInterval(() => {
    if (!busy) poll().catch(() => {});
  }, 500);
}
