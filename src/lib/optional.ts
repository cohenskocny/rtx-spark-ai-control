/**
 * Blackout is an optional feature. The guard script and its interpreter are
 * configuration, not an install step — a machine that only wants vLLM
 * management never needs them, and nothing here may suggest fetching one.
 *
 * These helpers keep the pages honest about that: one status for a pill, one
 * sentence for the page, and one reason for a control that cannot act.
 */

export interface BlackoutAvailability {
  /** Both the script and an interpreter are set and validate. */
  available: boolean;
  /** Short status, for a pill or a caption. */
  label: string;
  /** Why it is unavailable, empty when it is available. */
  reason: string;
}

export function blackoutAvailability(ready: boolean, missing: string[]): BlackoutAvailability {
  if (ready) {
    return { available: true, label: "blackout ready", reason: "" };
  }
  if (missing.length) {
    const list = missing.join(" and ");
    return {
      available: false,
      label: "blackout optional",
      reason: `Blackout is optional and not configured (${list}). vLLM management and monitoring do not need it; set both paths in Setup only if you use the OLED guard.`,
    };
  }
  return {
    available: false,
    label: "blackout unusable",
    reason:
      "Blackout is configured but the guard script or interpreter does not validate. Correct the path in Setup, or clear both to leave the feature off.",
  };
}

/** The text a disabled control explains itself with, or null when it may act. */
export function blackoutBlockedReason(availability: BlackoutAvailability): string | null {
  return availability.available ? null : availability.reason;
}
