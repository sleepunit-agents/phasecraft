// A visual projection of engine traces, never a source of playback decisions.
export function rings(trace, label = "Trigger") {
  if (!trace) return [];
  if (trace.type === "binary")
    return [...rings(trace.a, `${label} A`), ...rings(trace.b, `${label} B`)];
  return [{ ...trace, label }];
}
export function pulseAt(index, steps, pulses, rotation) {
  const phase = (((index - rotation) % steps) + steps) % steps;
  return (phase * pulses) % steps < pulses;
}
export function expression(trace) {
  if (!trace) return "";
  if (trace.type === "euclidean")
    return `E(${trace.pulses}, ${trace.steps})${trace.rotation ? ` ↻ ${trace.rotation}` : ""}`;
  if (trace.type === "part") return `${trace.id} · ${trace.mode}`;
  const op = {
    or: "OR",
    and: "AND",
    xor: "XOR",
    a_not_b: "A NOT B",
    b_not_a: "B NOT A",
  }[trace.op];
  return `(${expression(trace.a)} ${op} ${expression(trace.b)})`;
}
export function position(step) {
  if (step === null || step === undefined) return "1.1.1";
  return `${Math.floor(step / 16) + 1}.${Math.floor((step % 16) / 4) + 1}.${(step % 4) + 1}`;
}

export const operatorLabel = (op) =>
  ({
    or: "OR",
    and: "AND",
    xor: "XOR",
    a_not_b: "A NOT B",
    b_not_a: "B NOT A",
  })[op] || op;
// Interpolate only within the last authoritative step. Never invent later hits on a stalled IPC feed.
export function visualProgress(snapshot, elapsedMs, reducedMotion = false) {
  if (!snapshot?.playing || snapshot.step == null) return 0;
  return Math.min(
    1,
    snapshot.progress +
      (reducedMotion
        ? 0
        : Math.max(0, elapsedMs) / (60000 / snapshot.composition.tempo / 4)),
  );
}

// Select the latest source position that has actually been reached on each Part's clock.
export function visibleTraces(snapshot, progress = snapshot?.progress || 0) {
  if (!snapshot) return [];
  const tick = ((snapshot.step || 0) + progress) * 240;
  const chosen = new Map();
  for (const trace of snapshot.traces) {
    if (trace.tick <= tick || !snapshot.playing) chosen.set(trace.part, trace);
  }
  return [...chosen.values()];
}
