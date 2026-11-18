import assert from "node:assert/strict";
import test from "node:test";

import { blackoutAvailability, blackoutBlockedReason } from "../../src/lib/optional.ts";

test("a configured guard is simply ready", () => {
  const status = blackoutAvailability(true, []);
  assert.deepEqual(status, { available: true, label: "blackout ready", reason: "" });
  assert.equal(blackoutBlockedReason(status), null);
});

test("an unset guard reads as optional, not as a defect", () => {
  const status = blackoutAvailability(false, ["blackout engine", "blackout interpreter"]);
  assert.equal(status.available, false);
  assert.equal(status.label, "blackout optional");
  // Says what still works, and never suggests installing anything.
  assert.match(status.reason, /vLLM management and monitoring do not need it/);
  assert.match(status.reason, /blackout engine and blackout interpreter/);
  assert.doesNotMatch(status.reason, /install/i);
  assert.equal(blackoutBlockedReason(status), status.reason);
});

test("a guard that is set but does not validate is a different message", () => {
  const status = blackoutAvailability(false, []);
  assert.equal(status.label, "blackout unusable");
  assert.match(status.reason, /does not validate/);
  // The way out is offered, and it includes turning the feature off.
  assert.match(status.reason, /clear both/);
});

test("one missing half names only that half", () => {
  const status = blackoutAvailability(false, ["blackout interpreter"]);
  assert.match(status.reason, /\(blackout interpreter\)/);
  assert.doesNotMatch(status.reason, /blackout engine/);
});
