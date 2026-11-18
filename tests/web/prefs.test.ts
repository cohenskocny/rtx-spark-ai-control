import assert from "node:assert/strict";
import test from "node:test";

import { DEFAULT_PREFS, loadPrefs, savePrefs } from "../../src/lib/prefs.ts";

/** The window's localStorage, as far as these tests are concerned. */
function installStorage(behaviour: "ok" | "throw" = "ok") {
  const map = new Map<string, string>();
  (globalThis as unknown as { localStorage: Storage }).localStorage = {
    get length() {
      return map.size;
    },
    clear: () => map.clear(),
    key: (index: number) => [...map.keys()][index] ?? null,
    getItem: (key: string) => {
      if (behaviour === "throw") throw new Error("storage disabled");
      return map.get(key) ?? null;
    },
    setItem: (key: string, value: string) => {
      if (behaviour === "throw") throw new Error("storage disabled");
      map.set(key, value);
    },
    removeItem: (key: string) => {
      map.delete(key);
    },
  } as Storage;
  return map;
}

test("missing storage yields the documented defaults", () => {
  installStorage();
  assert.deepEqual(loadPrefs(), DEFAULT_PREFS);
});

test("preferences survive a round trip", () => {
  installStorage();
  savePrefs({ pollMs: 5000, pauseWhenHidden: false, reduceMotion: true, page: "updates" });
  assert.deepEqual(loadPrefs(), {
    pollMs: 5000,
    pauseWhenHidden: false,
    reduceMotion: true,
    page: "updates",
  });
});

test("the new pages are accepted, an unknown page falls back", () => {
  installStorage();
  for (const page of ["setup", "updates", "home", "logs"]) {
    savePrefs({ ...DEFAULT_PREFS, page });
    assert.equal(loadPrefs().page, page);
  }
  savePrefs({ ...DEFAULT_PREFS, page: "not-a-page" });
  assert.equal(loadPrefs().page, "home");
});

test("out of range values are corrected rather than trusted", () => {
  const map = installStorage();
  map.set(
    "rtx-v2.prefs.v1",
    JSON.stringify({ pollMs: 137, pauseWhenHidden: "yes", reduceMotion: 1, page: "server" }),
  );
  assert.deepEqual(loadPrefs(), {
    pollMs: DEFAULT_PREFS.pollMs,
    pauseWhenHidden: DEFAULT_PREFS.pauseWhenHidden,
    reduceMotion: DEFAULT_PREFS.reduceMotion,
    page: "server",
  });
});

test("corrupt storage does not throw", () => {
  const map = installStorage();
  map.set("rtx-v2.prefs.v1", "{not json");
  assert.deepEqual(loadPrefs(), DEFAULT_PREFS);
});

test("a storage that refuses writes does not throw", () => {
  installStorage("throw");
  assert.doesNotThrow(() => savePrefs(DEFAULT_PREFS));
  assert.deepEqual(loadPrefs(), DEFAULT_PREFS);
});
