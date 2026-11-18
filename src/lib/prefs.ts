/** Small, versioned preferences kept in the webview's localStorage. */

export interface Prefs {
  /** Telemetry poll interval in ms. 1000 / 2000 / 5000. */
  pollMs: number;
  /** Stop polling entirely while the window is minimised or hidden. */
  pauseWhenHidden: boolean;
  /** Disable non-essential animation. Mirrors the OS reduced-motion setting. */
  reduceMotion: boolean;
  /** Last page, so a relaunch lands where the operator left off. */
  page: string;
}

const KEY = "rtx-v2.prefs.v1";

export const DEFAULT_PREFS: Prefs = {
  pollMs: 2000,
  pauseWhenHidden: true,
  reduceMotion: false,
  page: "home",
};

const PAGES = ["home", "server", "blackout", "updates", "setup", "settings", "logs"];

export function loadPrefs(): Prefs {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return { ...DEFAULT_PREFS };
    const parsed = JSON.parse(raw) as Partial<Prefs>;
    return {
      pollMs: [1000, 2000, 5000].includes(Number(parsed.pollMs))
        ? Number(parsed.pollMs)
        : DEFAULT_PREFS.pollMs,
      pauseWhenHidden:
        typeof parsed.pauseWhenHidden === "boolean"
          ? parsed.pauseWhenHidden
          : DEFAULT_PREFS.pauseWhenHidden,
      reduceMotion:
        typeof parsed.reduceMotion === "boolean"
          ? parsed.reduceMotion
          : DEFAULT_PREFS.reduceMotion,
      page: PAGES.includes(String(parsed.page)) ? String(parsed.page) : DEFAULT_PREFS.page,
    };
  } catch {
    return { ...DEFAULT_PREFS };
  }
}

export function savePrefs(prefs: Prefs): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(prefs));
  } catch {
    /* storage disabled: preferences simply do not persist */
  }
}
