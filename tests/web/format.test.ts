import assert from "node:assert/strict";
import test from "node:test";

import { age, clockTime, gib, num, pct, span, throttleReasons, uptime } from "../../src/lib/format.ts";

test("num formats thousands and refuses null", () => {
  assert.equal(num(1234.5, 0), "1,235");
  assert.equal(num(1234.5, 1), "1,234.5");
  assert.equal(num(null), "—");
  assert.equal(num(undefined), "—");
  assert.equal(num(Number.NaN), "—");
});

test("pct and gib keep their unit", () => {
  assert.equal(pct(42), "42%");
  assert.equal(pct(42.4, 1), "42.4%");
  assert.equal(pct(null), "—");
  assert.equal(gib(2048), "2.0 GiB");
  assert.equal(gib(null), "—");
});

test("span picks the shortest honest unit", () => {
  assert.equal(span(45), "45s");
  assert.equal(span(180), "3m");
  assert.equal(span(330), "5m 30s");
  assert.equal(span(null), "—");
});

test("uptime separates days, hours and seconds", () => {
  assert.equal(uptime(30), "30s");
  assert.equal(uptime(3900), "1h 5m");
  assert.equal(uptime(90000), "1d 1h");
  assert.equal(uptime(null), "—");
});

test("age describes the distance from now", () => {
  assert.equal(age(null), "never");
  assert.equal(age(Date.now()), "just now");
  assert.equal(age(Date.now() - 30_000), "30s ago");
  assert.match(age(Date.now() - 300_000), /^5\.\dm ago$/);
});

test("clockTime renders a 24 hour clock", () => {
  const at = new Date(2026, 10, 18, 9, 5, 3).getTime();
  assert.equal(clockTime(at), "09:05:03");
  assert.equal(clockTime(0), "—");
});

test("throttleReasons ignores a clear mask and names the set bits", () => {
  assert.deepEqual(throttleReasons("0x0000000000000000"), []);
  assert.deepEqual(throttleReasons(""), []);
  assert.deepEqual(throttleReasons(null), []);
  assert.deepEqual(throttleReasons("0x0000000000000004"), ["sw power cap"]);
  // 0x64 = 0b110_0100: power cap, software thermal, hardware thermal.
  assert.deepEqual(throttleReasons("0x0000000000000064"), [
    "sw power cap",
    "sw thermal slowdown",
    "hw thermal slowdown",
  ]);
});
