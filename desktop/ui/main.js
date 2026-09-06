import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  rings,
  pulseAt,
  expression,
  position,
  operatorLabel,
  visualProgress,
} from "./rhythm.js";
import "./style.css";
import { setupController } from "./controllers.js";
import { setupUpdates } from "./updates.js";

const app = document.querySelector("#app");
app.innerHTML = `
<div class="titlebar"><div id="window-drag" class="window-drag">◉ <span>phasecraft</span></div><div class="window-controls"><button id="window-minimize" aria-label="Minimize window">─</button><button id="window-maximize" aria-label="Maximize or restore window">□</button><button id="window-close" aria-label="Close window">×</button></div></div>
<aside class="sidebar">
  <div class="brand"><span class="brand-mark">◉</span><div>phasecraft<small>MUSICAL SYSTEMS</small></div></div>
  <button id="projects-menu" aria-haspopup="dialog">Projects <span>⌄</span></button>
  <div class="eyebrow composition-heading">COMPOSITIONS</div><nav id="compositions"><p class="muted side-note">Open a project to explore its systems.</p></nav>

  <div class="sidebar-bottom"><span class="status-dot"></span> DETERMINISTIC BY DESIGN<small id="version">PLAYER / DEV</small></div>
</aside>
<main>
  <header><div><div class="eyebrow">PHASECRAFT / PLAYER</div><h1 id="title">Small systems.<br>Long stories.</h1><p id="subtitle">Open a folder. Find the groove. Watch it unfold.</p></div><div class="header-right"><button id="update-chip" class="update-chip" hidden></button><span id="watch-status" class="pill">TOML → LIVE MIDI</span><span id="part-count"></span></div></header>
  <p id="section-position" class="muted" hidden></p><section class="transport" aria-label="Playback controls"><button id="play" class="play" disabled>▶ Play</button><button id="stop" disabled>■ Stop</button><div class="metric"><label>POSITION</label><strong id="position">1.1.1</strong></div><div class="metric"><label>BPM</label><strong id="tempo">—</strong></div><div class="metric seed"><label>SEED</label><strong id="seed">—</strong></div><button id="settings-open" aria-haspopup="dialog">⚙ Settings</button></section>
  <dialog id="projects-dialog" aria-labelledby="projects-title"><div class="dialog-heading"><h2 id="projects-title">Projects</h2><button id="projects-dismiss" aria-label="Dismiss projects">×</button></div><div class="project-actions"><button id="open" class="primary">↗ Open project</button><button id="new">＋ New project</button><button id="close-project" disabled>Close current project</button></div><p class="muted">Compositions are read from disk. Temporary controller edits are discarded on Stop or close.</p><p id="project-error" role="alert" hidden></p><div class="eyebrow">RECENT PROJECTS</div><nav id="recent"></nav></dialog>
  <dialog id="settings-dialog" aria-labelledby="settings-title"><div class="dialog-heading"><h2 id="settings-title">MIDI & transport</h2><button id="settings-dismiss" aria-label="Dismiss settings">×</button></div><div class="routing"><label for="destination">MIDI DESTINATION</label><div><select id="destination"><option value="">Choose an output…</option><option value="@silent">Silent preview</option></select><button id="refresh" title="Refresh MIDI outputs" aria-label="Refresh MIDI outputs">↻</button></div><label class="sync-control"><input id="send-clock" type="checkbox"> Send tempo & transport</label></div><p class="muted">Remembered for this project on this computer. Starts with config/midi.toml until you save an override.</p><p id="settings-status" role="status"></p><button id="settings-save" class="primary">Save settings</button></dialog>
  <div id="error" role="alert" hidden></div><div id="midi-error" class="notice" hidden></div><div id="reload-error" class="notice" hidden></div>
  <section id="welcome"><div class="welcome-rings" aria-hidden="true"><i></i><i></i><i></i><b>◉</b></div><div class="eyebrow">A FRONT PANEL FOR YOUR IDEAS</div><h2>Every cycle has<br>something to say.</h2><p>Independent rhythms, probability and emphasis.<br>Your musical systems, made visible.</p><button id="welcome-open" class="primary">Open a project folder ↗</button><button id="welcome-new">New project</button><div id="welcome-recent"></div><p class="welcome-hint">New here? New project includes 909 techno, DnB and garage.</p></section>
  <section id="system" hidden><div class="section-heading"><div><span class="eyebrow">THE SYSTEM</span><h2>Independent cycles. Shared time.</h2></div><div class="legend"><span><i class="hit"></i>Hit</span><span><i class="accent"></i>Accent</span><span><i class="rejected"></i>Omitted</span></div></div><div id="cards"></div>
  <section id="detail" hidden><div class="detail-heading"><div class="eyebrow">UNDER THE SURFACE</div><h2 id="detail-title"></h2><button id="detail-close" aria-label="Close inspector">×</button></div><div id="detail-body"></div></section></section>
  <footer><span id="state">READY WHEN YOU ARE</span><span>Files are the score. This is the player.</span></footer>
</main>`;
setupController(invoke);
const $ = (id) => document.getElementById(id);
let project = null,
  snapshot = null,
  selectedPart = null,
  busy = false,
  polling = false;
let cards = new Map(),
  recent = [],
  platform = "",
  lastModel = "",
  receivedAt = performance.now(),
  settingsDraft = null;
const basename = (path) => path.split(/[\\/]/).filter(Boolean).pop();
const human = (id) => id.replaceAll("_", " ");
function error(message) {
  $("error").hidden = !message;
  $("error").textContent = message || "";
  $("project-error").textContent = message || "";
  $("project-error").hidden = !message;
}
async function action(fn) {
  if (busy) return;
  busy = true;
  error(null);
  controls();
  try {
    await fn();
    await poll();
  } catch (e) {
    error(String(e));
  } finally {
    busy = false;
    controls();
  }
}
function controls() {
  const playing = snapshot?.playing || false;
  $("play").disabled = busy || playing || !project || !$("destination").value;
  $("stop").disabled = busy || !playing;
  $("destination").disabled = busy || playing;
  $("refresh").disabled = busy || playing;
  $("send-clock").disabled = busy || playing;
  $("settings-open").disabled = busy || !project;
  $("settings-save").disabled = busy || playing || !project;
  $("close-project").disabled = busy || !project;
  $("projects-menu").disabled = busy;
  $("open").disabled = busy;
  $("new").disabled = busy;
  $("welcome-open").disabled = busy;
  for (const b of $("compositions").querySelectorAll("button"))
    b.disabled = busy || playing;
}
function renderRecent() {
  $("recent").replaceChildren();
  $("welcome-recent").replaceChildren();
  for (const path of recent) {
    const b = document.createElement("button");
    b.className = "nav-item recent";
    b.textContent = basename(path);
    b.title = path;
    b.onclick = () => action(() => openPath(path));
    $("recent").append(b);
    const home = b.cloneNode(true);
    home.onclick = b.onclick;
    $("welcome-recent").append(home);
  }
}
async function openPath(path, create = false) {
  const result = await invoke(create ? "new_project" : "open_project", {
    path,
  });
  project = result.project;
  $("projects-dialog").close();
  document.body.classList.add("project-open");
  selectedPart = null;
  ((lastModel = ""), (receivedAt = performance.now()));
  recent = [project.path, ...recent.filter((p) => p !== project.path)].slice(
    0,
    8,
  );
  renderRecent();
  $("title").textContent = project.name;
  $("subtitle").textContent = project.path;
  $("welcome").hidden = true;
  $("system").hidden = false;
  $("compositions").replaceChildren();
  for (const path of project.compositions) {
    const b = document.createElement("button");
    b.className = "nav-item";
    b.textContent = basename(path).replace(/\.toml$/, "");
    b.dataset.path = path;
    b.classList.toggle("selected", path === result.selected);
    b.onclick = () =>
      action(async () => {
        await invoke("select_composition", { path });
        selectedPart = null;
        ((lastModel = ""), (receivedAt = performance.now()));
        for (const item of $("compositions").children)
          item.classList.toggle("selected", item === b);
      });
    $("compositions").append(b);
  }
  $("send-clock").checked = result.send_clock || false;
  const wanted = result.silent
    ? "@silent"
    : result.virtual_port
      ? "@virtual"
      : result.port || "";
  if (wanted && ![...$("destination").options].some((o) => o.value === wanted))
    $("destination").add(new Option(`${wanted} (unavailable)`, wanted));
  $("destination").value = wanted;
  $("watch-status").textContent = "WATCHING TOML";
}
function closeInspector() {
  selectedPart = null;
  renderDetail();
}
$("detail-close").onclick = closeInspector;
$("projects-menu").onclick = () => $("projects-dialog").showModal();
$("projects-dismiss").onclick = () => $("projects-dialog").close();
$("settings-open").onclick = () => {
  settingsDraft = {
    destination: $("destination").value,
    sendClock: $("send-clock").checked,
  };
  $("settings-status").textContent = "";
  $("settings-dialog").showModal();
};
$("settings-dismiss").onclick = () => $("settings-dialog").close();
$("settings-dialog").addEventListener("close", () => {
  if (settingsDraft) {
    $("destination").value = settingsDraft.destination;
    $("send-clock").checked = settingsDraft.sendClock;
    settingsDraft = null;
    controls();
  }
});
for (const id of ["projects-dialog", "settings-dialog"]) {
  $(id).addEventListener("click", (e) => {
    if (e.target === $(id)) {
      const r = $(id).getBoundingClientRect();
      if (
        e.clientX < r.left ||
        e.clientX > r.right ||
        e.clientY < r.top ||
        e.clientY > r.bottom
      )
        $(id).close();
    }
  });
}
$("close-project").onclick = () =>
  action(async () => {
    await invoke("close_project");
    project = null;
    snapshot = null;
    selectedPart = null;
    lastModel = "";
    cards.clear();
    $("projects-dialog").close();
    document.body.classList.remove("project-open");
    $("welcome").hidden = false;
    $("system").hidden = true;
    $("detail").hidden = true;
    $("compositions").replaceChildren();
    $("title").textContent = "Small systems. Long stories.";
    $("subtitle").textContent =
      "Open a folder. Find the groove. Watch it unfold.";
    $("position").textContent = "1.1.1";
    $("tempo").textContent = "—";
    $("seed").textContent = "—";
    $("part-count").textContent = "";
    $("section-position").hidden = true;
    $("reload-error").hidden = true;
    $("watch-status").textContent = "TOML → LIVE MIDI";
    $("state").textContent = "READY WHEN YOU ARE";
    $("state").classList.remove("running");
  });
$("settings-save").onclick = () =>
  action(async () => {
    const output = $("destination").value;
    try {
      await invoke("save_settings", {
        routing: {
          port: output && !output.startsWith("@") ? output : null,
          virtual_port: output === "@virtual",
          silent: output === "@silent",
          send_clock: $("send-clock").checked,
        },
      });
    } catch (e) {
      $("settings-status").textContent = String(e);
      throw e;
    }
    settingsDraft = null;
    $("settings-status").textContent = "Saved for this project.";
    $("settings-dialog").close();
  });
for (const name of ["minimize", "maximize", "close"])
  $("window-" + name).onclick = () =>
    invoke("window_control", { action: name }).catch((e) => error(String(e)));
$("window-drag").onmousedown = (e) => {
  if (e.button === 0)
    invoke("window_control", {
      action: e.detail === 2 ? "maximize" : "drag",
    }).catch((e) => error(String(e)));
};
async function chooseProject() {
  const path = await open({
    directory: true,
    multiple: false,
    title: "Open a Phasecraft project",
  });
  if (path) await openPath(path);
}
$("open").onclick = () => action(chooseProject);
$("welcome-open").onclick = () => action(chooseProject);
$("new").onclick = () =>
  action(async () => {
    const path = await save({
      title: "Choose a name for the new project folder",
      defaultPath: "my-set",
    });
    if (path) await openPath(path, true);
  });
$("welcome-new").onclick = () => $("new").click();
async function refreshPorts() {
  const previous = $("destination").value;
  $("destination").replaceChildren(
    new Option("Choose an output…", ""),
    new Option("Silent preview", "@silent"),
  );
  if (platform !== "windows-x64")
    $("destination").add(new Option("Virtual MIDI · Phasecraft", "@virtual"));
  try {
    for (const name of await invoke("destinations"))
      $("destination").add(new Option(name, name));
    $("midi-error").hidden = true;
  } catch (e) {
    $("midi-error").textContent =
      `MIDI outputs unavailable. Silent preview still works. ${e}`;
    $("midi-error").hidden = false;
  }
  if (
    previous &&
    ![...$("destination").options].some((o) => o.value === previous)
  )
    $("destination").add(new Option(`${previous} (unavailable)`, previous));
  $("destination").value = previous;
  controls();
}
$("refresh").onclick = () => action(refreshPorts);
$("destination").onchange = controls;
$("play").onclick = () =>
  action(async () => {
    const output = $("destination").value;
    await invoke("start", {
      port: output.startsWith("@") ? null : output,
      virtualPort: output === "@virtual",
      silent: output === "@silent",
      sendClock: $("send-clock").checked,
    });
  });
$("stop").onclick = () => action(() => invoke("stop"));
document.addEventListener("keydown", (e) => {
  if (e.code === "Escape") closeInspector();
  if (document.querySelector("dialog[open]")) return;
  if (
    e.code === "Space" &&
    !["INPUT", "SELECT", "BUTTON", "TEXTAREA"].includes(
      document.activeElement.tagName,
    )
  ) {
    e.preventDefault();
    (snapshot?.playing ? $("stop") : $("play")).click();
  }
});

function ensureCards(c) {
  const model = JSON.stringify(
    c.parts.map((p) => ({ id: p.id, output: p.output })),
  );
  if (model === lastModel) return;
  lastModel = model;
  cards.clear();
  $("cards").replaceChildren();
  for (const [i, part] of c.parts.entries()) {
    const card = document.createElement("button");
    card.className = "part-card";
    card.dataset.part = part.id;
    const head = document.createElement("div");
    head.className = "part-head";
    const title = document.createElement("h3");
    title.textContent = human(part.id);
    const number = document.createElement("span");
    number.textContent = String(i + 1).padStart(2, "0");
    number.className = "part-number";
    head.append(number, title);
    const badge = document.createElement("span");
    badge.className = "activity";
    head.append(badge);
    const route = document.createElement("p");
    route.className = "part-route";
    route.textContent = `CH ${part.output.channel} / NOTE ${part.output.note}`;
    const canvas = document.createElement("canvas");
    canvas.setAttribute(
      "aria-label",
      `Rhythm cycles and resolved output for ${human(part.id)}`,
    );
    const caption = document.createElement("div");
    caption.className = "card-caption";
    card.append(head, route, canvas, caption);
    card.onclick = () => {
      selectedPart = selectedPart === part.id ? null : part.id;
      renderDetail();
    };
    $("cards").append(card);
    const formula = document.createElement("div");
    formula.className = "card-formula";
    card.insertBefore(formula, caption);
    cards.set(part.id, { card, canvas, badge, caption, formula });
  }
}
function drawRing(ctx, r, x, y, radius, live, progress, accent, resolved) {
  const color = accent ? "#f6b980" : "#93d5c6";
  if (r.type === "part") {
    ctx.fillStyle = "#8d999c";
    ctx.font = "11px monospace";
    ctx.textAlign = "center";
    ctx.fillText(`↗ ${r.id}`, x, y);
    ctx.fillText(r.mode, x, y + 16);
    return;
  }
  ctx.beginPath();
  ctx.arc(x, y, radius, 0, Math.PI * 2);
  ctx.strokeStyle = "#293335";
  ctx.lineWidth = 1;
  ctx.stroke();
  // Large valid cycles remain bounded visually; labels keep the exact cycle length.
  const stride = Math.max(1, Math.ceil(r.steps / 128));
  for (let index = 0; index < r.steps; index += stride) {
    const theta = (index / r.steps) * Math.PI * 2 - Math.PI / 2;
    const on = pulseAt(index, r.steps, r.pulses, r.rotation);
    const current =
      live &&
      progress < 0.3 &&
      Math.floor(r.phase / stride) === Math.floor(index / stride);
    ctx.beginPath();
    ctx.arc(
      x + Math.cos(theta) * radius,
      y + Math.sin(theta) * radius,
      current ? 4.5 : on ? 3 : 2,
      0,
      Math.PI * 2,
    );
    ctx.fillStyle =
      current && on && resolved ? "#ffffff" : on ? color : "#344043";
    if (current && on && !resolved) {
      ctx.strokeStyle = "#93a6a1";
      ctx.lineWidth = 1.5;
      ctx.stroke();
    } else ctx.fill();
  }
  if (live) {
    const theta = ((r.phase + progress) / r.steps) * Math.PI * 2 - Math.PI / 2;
    ctx.beginPath();
    ctx.arc(
      x + Math.cos(theta) * radius,
      y + Math.sin(theta) * radius,
      3.5,
      0,
      Math.PI * 2,
    );
    ctx.fillStyle = "#eef6f1";
    ctx.fill();
  }
  ctx.textAlign = "center";
  ctx.fillStyle = "#edf2ea";
  ctx.font = "500 15px monospace";
  ctx.fillText(`${r.pulses}/${r.steps}`, x, y + 5);
  ctx.fillStyle = "#8c999c";
  ctx.font = "10px monospace";
  ctx.fillText(
    r.label.replace("Trigger", "TRIG").replace("Accent", "ACC"),
    x,
    y + radius + 21,
  );
}
function drawCard(view, trace, progress = snapshot.progress) {
  const all = [
    ...rings(trace.trigger.rhythm),
    ...rings(trace.accent.rhythm, "Accent"),
    ...(trace.shared_accents || []).flatMap((s) =>
      rings(s.decision.rhythm, `Shared ${human(s.name)}`),
    ),
  ];
  const visible = all.slice(0, 8);
  const columns = Math.min(3, visible.length);
  const rows = Math.ceil(visible.length / columns);
  const width = view.canvas.clientWidth || 300,
    height = rows * 110 + 76,
    scale = window.devicePixelRatio || 1;
  if (
    view.canvas.width !== Math.round(width * scale) ||
    view.canvas.height !== Math.round(height * scale)
  ) {
    view.canvas.width = Math.round(width * scale);
    view.canvas.height = Math.round(height * scale);
  }
  view.canvas.style.height = `${height}px`;
  const ctx = view.canvas.getContext("2d");
  ctx.setTransform(scale, 0, 0, scale, 0, 0);
  ctx.clearRect(0, 0, width, height);
  const cell = width / columns,
    radius = Math.min(30, cell / 2 - 17);
  const sounded =
    !!trace.event && progress >= (trace.event.groove?.offset_ticks || 0) / 240;
  visible.forEach((r, i) =>
    drawRing(
      ctx,
      r,
      cell * ((i % columns) + 0.5),
      45 + Math.floor(i / columns) * 110,
      radius,
      snapshot.playing && snapshot.step !== null,
      progress,
      !r.label.startsWith("Trigger"),
      sounded && (r.label.startsWith("Trigger") || trace.event?.accent.active),
    ),
  );
  // Inline operators are placed only between direct sibling leaves, never across unrelated lanes.
  let leaf = 0;
  function drawOperators(node) {
    if (node.type !== "binary") {
      leaf++;
      return;
    }
    const first = leaf;
    drawOperators(node.a);
    const middle = leaf;
    drawOperators(node.b);
    if (
      node.a.type !== "binary" &&
      node.b.type !== "binary" &&
      middle < visible.length &&
      Math.floor(first / columns) === Math.floor(middle / columns)
    ) {
      ctx.fillStyle = node.active ? "#b9e4d5" : "#82928e";
      ctx.font = "8px monospace";
      ctx.textAlign = "center";
      const label =
        { a_not_b: "A−B", b_not_a: "B−A" }[node.op] || operatorLabel(node.op);
      ctx.fillText(
        label,
        cell * middle,
        45 + Math.floor(first / columns) * 110,
      );
    }
  }
  drawOperators(trace.trigger.rhythm);
  view.formula.textContent = expression(trace.trigger.rhythm);
  view.formula.title =
    "Trigger relationship · hollow highlight means active input without a resolved note";
  const historyY = rows * 110 + 32;
  const history = snapshot.history.slice(-16),
    gap = 5,
    box = (width - gap * 15) / 16;
  for (let i = 0; i < 16; i++) {
    let item = history[i - (16 - history.length)]?.parts.find(
      (p) => p.id === trace.part,
    );
    if (
      snapshot.playing &&
      history[i - (16 - history.length)]?.step === snapshot.step &&
      progress < (trace.event?.groove?.offset_ticks || 0) / 240
    )
      item = null;
    ctx.fillStyle = item?.accented
      ? "#f6b980"
      : item?.fired
        ? "#93d5c6"
        : "#263133";
    ctx.fillRect(i * (box + gap), historyY, box, 11);
    if (item?.eligible && !item.fired) {
      ctx.strokeStyle = "#89979a";
      ctx.beginPath();
      ctx.moveTo(i * (box + gap) + 2, historyY + 2);
      ctx.lineTo(i * (box + gap) + box - 2, historyY + 9);
      ctx.stroke();
    }
  }
  ctx.fillStyle = "#758589";
  ctx.textAlign = "left";
  ctx.font = "9px monospace";
  ctx.fillText("RECENT RESOLVED OUTPUT", 0, historyY + 35);
  const hit =
    snapshot.playing &&
    snapshot.step !== null &&
    trace.event &&
    progress >= (trace.event.groove?.offset_ticks || 0) / 240 &&
    progress < (trace.event.groove?.offset_ticks || 0) / 240 + 0.65;
  view.card.classList.toggle("fired", !!hit);
  view.card.classList.toggle("accented", !!(hit && trace.event.accent.active));
  view.card.classList.toggle("selected-part", trace.part === selectedPart);
  view.badge.textContent = hit
    ? trace.event.groove?.ghost
      ? "GHOST"
      : trace.event.accent.active
        ? "ACCENT"
        : "HIT"
    : snapshot.playing
      ? "RUNNING"
      : "READY";
  view.caption.textContent = `${Math.round(trace.trigger.probability * 100)}% trigger admission · ${all.length > 3 ? `${all.length} lanes · inspect all ↗` : "Inspect ↗"}`;
}
function renderDetail() {
  const trace = snapshot?.traces.find((t) => t.part === selectedPart);
  $("detail").hidden = !trace;
  if (!trace) return;
  $("detail-title").textContent = human(selectedPart);
  $("detail-body").replaceChildren();
  for (const [label, lane] of [
    ["Trigger", trace.trigger],
    ["Accent", trace.accent],
    ...(trace.shared_accents || []).map((s) => [
      `Shared accent: ${human(s.name)} (amount ${s.amount.toFixed(2)})`,
      s.decision,
    ]),
  ]) {
    const block = document.createElement("div");
    block.className = "lane-detail";
    const title = document.createElement("h4");
    title.textContent = label;
    const formula = document.createElement("code");
    formula.textContent = expression(lane.rhythm);
    const detail = document.createElement("p");
    detail.textContent = `Admission ${(lane.probability * 100).toFixed(0)}% · roll ${lane.roll.toFixed(3)} · ${lane.admitted ? "passed" : "not admitted"}`;
    block.append(title, formula, detail);
    for (const r of rings(lane.rhythm, label)) {
      const p = document.createElement("p");
      p.className = "muted";
      p.textContent =
        r.type === "euclidean"
          ? `${r.label}: ${r.steps} steps / ${r.pulses} pulses / phase ${r.phase} / rotation ${r.rotation}${r.steps > 128 ? " · ring display sampled" : ""}`
          : `${r.label}: ${r.id} (${r.mode})`;
      block.append(p);
    }
    $("detail-body").append(block);
  }
  if (trace.event?.groove) {
    const g = trace.event.groove;
    const block = document.createElement("div");
    block.className = "lane-detail";
    const heading = document.createElement("h4");
    heading.textContent = "Groove";
    const detail = document.createElement("p");
    detail.textContent = `Timing +${g.offset_ticks} ticks · base velocity ×${g.velocity_factor.toFixed(3)} · ${g.ghost ? "ghost hit" : "normal hit"} · ghost roll ${g.ghost_roll.toFixed(3)}`;
    if (g.touch) {
      detail.textContent += ` · offbeat ×${g.touch.offbeat_factor.toFixed(2)} · gap ×${g.touch.gap_factor.toFixed(2)} · touch ×${g.touch.velocity_jitter_factor.toFixed(3)} · requested jitter ${g.touch.requested_jitter_ticks} ticks`;
    }
    const context = document.createElement("p");
    context.textContent = `Run context: ${g.run_before} before / ${g.run_after} after (up to 2 each) · gate ${trace.event.duration_ticks}/${g.requested_gate_ticks} ticks`;
    block.append(heading, detail, context);
    $("detail-body").append(block);
  }
  if (trace.event?.controls?.length) {
    const block = document.createElement("div");
    block.className = "lane-detail";
    const heading = document.createElement("h4");
    heading.textContent = "Accent controls";
    block.append(heading);
    for (const control of trace.event.controls) {
      const detail = document.createElement("p");
      detail.textContent = `${human(control.name)} · ${control.amount.toFixed(3)} → channel ${control.channel} CC ${control.cc}: ${control.value} · reset ${control.reset} at note-off / Stop`;
      block.append(detail);
    }
    $("detail-body").append(block);
  }
  if (trace.parameters?.length) {
    const block = document.createElement("div");
    block.className = "lane-detail";
    const heading = document.createElement("h4");
    heading.textContent = "Parameters";
    block.append(heading);
    const tick = trace.tick + (snapshot.playing ? snapshot.progress * 240 : 0);
    for (const parameter of trace.parameters) {
      const sample =
        parameter.samples.filter((s) => s.tick <= tick).at(-1) ||
        parameter.samples[0];
      const detail = document.createElement("p");
      const memory = sample.envelope
        ? ` · envelope ${sample.envelope.level.toFixed(3)} from ${sample.envelope.impulses} impulses`
        : "";
      const motion = sample.automation
        ? ` · segment ${sample.automation.segment}, cycle ${sample.automation.cycle + 1} (${human(sample.automation.curve)})`
        : "";
      detail.textContent = `${human(parameter.name)} · base ${sample.base.toFixed(3)} · emphasis ${sample.emphasis.toFixed(3)} → ${sample.amount.toFixed(3)} · channel ${parameter.channel} CC ${parameter.cc}: ${sample.value}${motion}${memory}`;
      block.append(detail);
    }
    const policy = document.createElement("p");
    policy.className = "muted";
    policy.textContent =
      "Runs through rests · held through notes · kit default restored on Stop";
    block.append(policy);
    $("detail-body").append(block);
  }
  const result = document.createElement("p");
  result.className = "resolved";
  result.textContent =
    snapshot.step === null
      ? "Preview at the starting position. Press Play to begin."
      : trace.event
        ? `Resolved note · emphasis ${trace.event.accent.amount.toFixed(2)} · ${trace.event.duration_ticks} musical ticks`
        : "Resolved rest";
  $("detail-body").append(result);
}
function render() {
  controls();
  if (!snapshot?.composition) return;
  const c = snapshot.composition;
  ensureCards(c);
  $("watch-status").textContent = snapshot.playing
    ? "WATCHING TOML"
    : "FILES LOADED";
  $("tempo").textContent = c.tempo;
  $("seed").textContent = snapshot.seed_label || c.seed;
  $("position").textContent = position(snapshot.step);
  const section = snapshot.traces[0]?.section;
  $("section-position").hidden = !section;
  $("section-position").textContent = section
    ? `${section.phrase} · section ${section.index}/${section.count} · bar ${section.bar}/${section.bars} · cycle ${section.cycle + 1} · ${section.phase} phase`
    : "";
  $("part-count").textContent =
    `${c.parts.length} PARTS / ${c.phrase_bars}-BAR PHRASE`;
  $("state").textContent = snapshot.playing
    ? $("destination").value === "@silent"
      ? "● SILENT PREVIEW · NO MIDI SENT"
      : "● PLAYING · LIVE MIDI"
    : "■ STOPPED";
  $("state").classList.toggle("running", snapshot.playing);
  $("reload-error").hidden = !snapshot.reload_error;
  $("reload-error").textContent = snapshot.reload_error
    ? `Edit rejected. Playing the last valid system. ${snapshot.reload_error}`
    : "";
  if (snapshot.error) error(snapshot.error);
  for (const trace of snapshot.traces) {
    const view = cards.get(trace.part);
    if (view) drawCard(view, trace);
  }
  renderDetail();
}
async function poll() {
  if (polling) return;
  polling = true;
  try {
    snapshot = await invoke("snapshot");
    receivedAt = performance.now();
    render();
  } catch (e) {
    error(String(e));
  } finally {
    polling = false;
  }
}
(async () => {
  try {
    const data = await invoke("initial");
    recent = data.recent;
    platform = data.version.platform;
    document.body.classList.toggle("macos", platform.startsWith("macos"));
    $("version").textContent = `DEV / ${data.version.commit.slice(0, 7)}`;
    renderRecent();
    await refreshPorts();
    await poll();
    setupUpdates(invoke, $("update-chip"), $("version"), action);
  } catch (e) {
    error(`Could not connect to the player: ${e}`);
  }
})();
setInterval(poll, 40);
window.addEventListener("resize", render);

const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
function animate(now) {
  if (snapshot?.playing && !document.hidden) {
    const progress = visualProgress(
      snapshot,
      now - receivedAt,
      reducedMotion.matches,
    );
    for (const trace of snapshot.traces) {
      const view = cards.get(trace.part);
      if (view) drawCard(view, trace, progress);
    }
  }
  requestAnimationFrame(animate);
}
requestAnimationFrame(animate);
