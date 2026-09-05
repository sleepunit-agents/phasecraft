import test from "node:test";
import assert from "node:assert/strict";
import { pulseAt, rings, position, expression } from "./rhythm.js";
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
