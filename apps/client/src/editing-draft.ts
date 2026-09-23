import type { JsonValue, Page } from "@honkoku/client-api/types";
export interface LocalDraft {
  source: string;
  draft: string;
  notes?: (JsonValue | null)[];
  acknowledgedNotes?: (JsonValue | null)[];
  updatedAt?: string | null;
}
function timestamp(value?: string | null): bigint | undefined {
  if (!value) return;
  const ms = Date.parse(value);
  if (!Number.isFinite(ms)) return;
  const fraction = value.match(/\.(\d+)/)?.[1] ?? "";
  return BigInt(ms) * 1_000_000n + BigInt(fraction.padEnd(9, "0").slice(3, 9));
}
/** Note edits this device has not seen acknowledged by the server. */
export function notesPending(
  notes: (JsonValue | null)[],
  acknowledged: (JsonValue | null)[],
): boolean {
  return JSON.stringify(notes) !== JSON.stringify(acknowledged);
}
export function restoreDraft(page: Page, local?: LocalDraft) {
  const serverTime = timestamp(page.updatedAt),
    localTime = timestamp(local?.updatedAt);
  const current =
    local &&
    localTime !== undefined &&
    serverTime !== undefined &&
    localTime >= serverTime;
  const unsavedLocal = current && local.source !== local.draft;
  const serverNotes = JSON.parse(
    JSON.stringify(page.tempNotes ?? page.notes),
  ) as (JsonValue | null)[];
  const localNotes = current && local.notes ? local.notes : undefined;
  const localAck = current ? local.acknowledgedNotes : undefined;
  // Note writes leave updatedAt alone, so a local array that is already fully
  // sent can still be an older generation than the server's. Keep local notes
  // only while an edit is still unacknowledged.
  const unsentNotes =
    localNotes !== undefined &&
    JSON.stringify(localNotes) !== JSON.stringify(localAck ?? serverNotes);
  return {
    source:
      local && unsavedLocal ? local.source : (page.tempText ?? page.text),
    notes: unsentNotes ? localNotes! : serverNotes,
    acknowledgedNotes: JSON.parse(
      JSON.stringify(unsentNotes ? (localAck ?? serverNotes) : serverNotes),
    ) as (JsonValue | null)[],
    unsavedLocal: Boolean(unsavedLocal),
  };
}
