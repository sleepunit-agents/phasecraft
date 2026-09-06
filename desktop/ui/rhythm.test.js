import test from "node:test";
import assert from "node:assert/strict";
import {
  pulseAt,
  rings,
  position,
  expression,
  visualProgress,
} from "./rhythm.js";
test("ring uses the engine Euclidean convention including signed rotation", () => {
  assert.equal(
    Array.from({ length: 8 }, (_, i) => (pulseAt(i, 8, 3, 0) ? "x" : ".")).join(
      "",
    ),
    "x..x..x.",
  );
  for (let steps = 1; steps < 65; steps++)
    for (let pulses = 0; pulses <= steps; pulses++)
      for (const rotation of [-2, 0, 3]) {
        assert.equal(
          Array.from({ length: steps }, (_, i) =>
            pulseAt(i, steps, pulses, rotation),
          ).filter(Boolean).length,
          pulses,
        );
      }
});
test("independent cycles and references retain their own identities", () => {
  const tree = {
    type: "binary",
    op: "a_not_b",
    a: { type: "euclidean", steps: 5, pulses: 2, rotation: 0, phase: 2 },
    b: { type: "part", id: "kick", mode: "hits" },
  };
  assert.equal(rings(tree)[0].phase, 2);
  assert.equal(rings(tree)[1].id, "kick");
  assert.match(expression(tree), /A NOT B/);
  assert.equal(position(64), "5.1.1");
  assert.equal(position(null), "1.1.1");
});

test("cursor interpolates a sixteenth at tempo and stops at stale step boundary", () => {
  const snapshot = {
    playing: true,
    step: 0,
    progress: 0,
    composition: { tempo: 120 },
  };
  assert.equal(visualProgress(snapshot, 62.5), 0.5);
  assert.equal(visualProgress(snapshot, 125), 1);
  assert.equal(visualProgress(snapshot, 5000), 1);
  assert.equal(visualProgress({ ...snapshot, playing: false }, 62.5), 0);
  assert.equal(visualProgress(snapshot, 62.5, true), 0);
});
