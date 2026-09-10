import type { JsonValue, Page } from "@honkoku/client-api/types";
export interface LocalDraft {
  source: string;
  draft: string;
  notes?: (JsonValue | null)[];
  updatedAt?: string | null;
}
function timestamp(value?: string | null): bigint | undefined {
  if (!value) return;
  const ms = Date.parse(value);
  if (!Number.isFinite(ms)) return;
  const fraction = value.match(/\.(\d+)/)?.[1] ?? "";
  return BigInt(ms) * 1_000_000n + BigInt(fraction.padEnd(9, "0").slice(3, 9));
}
export function restoreDraft(page: Page, local?: LocalDraft) {
  const serverTime = timestamp(page.updatedAt),
    localTime = timestamp(local?.updatedAt);
  const current =
    local &&
    localTime !== undefined &&
    serverTime !== undefined &&
    localTime >= serverTime;
  return {
    source:
      current && local.source !== local.draft
        ? local.source
        : (page.tempText ?? page.text),
    notes:
      current && local.notes
        ? local.notes
        : (JSON.parse(
            JSON.stringify(page.tempNotes ?? page.notes),
          ) as (JsonValue | null)[]),
  };
}
