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
