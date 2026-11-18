import assert from "node:assert/strict";
import test from "node:test";

import {
  checkPill,
  compatPill,
  day,
  quota,
  readableNotes,
  stamp,
} from "../../src/lib/present.ts";

test("day keeps a plain date and passes junk through", () => {
  assert.equal(day("2026-11-07T20:10:43Z"), "2026-11-07");
  assert.equal(day(""), "—");
  assert.equal(day("not a date"), "not a date");
});

test("stamp treats zero as nothing recorded", () => {
  assert.equal(stamp(0), "—");
  assert.equal(stamp(null), "—");
  assert.match(stamp(Date.UTC(2026, 10, 18, 10, 47, 24)), /^2026-11-18 \d\d:\d\d:\d\d$/);
});

test("a check status is not a compatibility status", () => {
  assert.deepEqual(checkPill("ok"), { tone: "live", text: "reachable" });
  assert.deepEqual(checkPill("offline"), { tone: "offline", text: "offline" });
  assert.deepEqual(checkPill("rate-limited"), { tone: "loading", text: "rate limited" });
  assert.deepEqual(checkPill("error"), { tone: "offline", text: "check failed" });
  assert.deepEqual(checkPill("never"), { tone: "idle", text: "not checked" });
  // An unknown status must not be dressed up as good news.
  assert.deepEqual(checkPill("something-new"), { tone: "idle", text: "not checked" });
});

test("only the tested baseline reads as compatible", () => {
  assert.deepEqual(compatPill("current"), { tone: "live", text: "compatible" });
  assert.deepEqual(compatPill("update-available"), { tone: "loading", text: "newer upstream" });
  assert.deepEqual(compatPill("ahead"), { tone: "idle", text: "local ahead" });
  assert.deepEqual(compatPill("unsupported-platform"), {
    tone: "offline",
    text: "not supported",
  });
  assert.deepEqual(compatPill("unknown"), { tone: "idle", text: "unknown" });
});

test("release notes lose markdown that cannot render", () => {
  const raw = [
    "## Highlights",
    "- [Opt-in local monitor](https://github.com/x/y/commit/abc) is here",
    "- **Bold** claim",
    "```python",
    "vllm serve",
    "```",
    "**Full Changelog**: https://github.com/x/y/compare/a...b",
  ].join("\n");
  const text = readableNotes(raw);
  assert.match(text, /## Highlights/);
  assert.match(text, /- Opt-in local monitor is here/);
  assert.match(text, /- Bold claim/);
  assert.match(text, /vllm serve/);
  assert.doesNotMatch(text, /\]\(/);
  assert.doesNotMatch(text, /\*\*/);
  assert.doesNotMatch(text, /```/);
  assert.doesNotMatch(text, /Full Changelog/);
});

test("notes collapse blank runs", () => {
  assert.equal(readableNotes("a\n\n\n\nb"), "a\n\nb");
  assert.equal(readableNotes("   "), "");
});

test("quota never invents a number", () => {
  assert.equal(quota(50, 60), "50 of 60 left");
  assert.equal(quota(0, 60), "0 of 60 left");
  assert.equal(quota(null, 60), "—");
  assert.equal(quota(10, null), "10 of ? left");
});
