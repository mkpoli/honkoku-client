import type { JsonValue } from "@honkoku/client-api/types";

export interface HistoryVersion {
  source: string;
  notes?: (JsonValue | null)[];
}

export function historyVersion(data?: JsonValue): HistoryVersion {
  const snapshot =
    data && typeof data === "object" && !Array.isArray(data) ? data : {};
  // Save events contain the page before saving: tempNotes belongs to this
  // version, while notes can still belong to the previous save.
  const notes = Array.isArray(snapshot.tempNotes)
    ? snapshot.tempNotes
    : Array.isArray(snapshot.notes)
      ? snapshot.notes
      : undefined;
  return {
    source: typeof snapshot.text === "string" ? snapshot.text : "",
    notes,
  };
}

export function restoreHistoryDraft(
  version: HistoryVersion,
  currentNotes: (JsonValue | null)[],
) {
  return {
    source: version.source,
    notes:
      version.notes === undefined
        ? currentNotes
        : (JSON.parse(JSON.stringify(version.notes)) as (JsonValue | null)[]),
  };
}
