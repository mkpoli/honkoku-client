import type { JsonValue, Page, PageNote } from "@honkoku/client-api/types";
export interface NoteFormDraft {
  note: PageNote;
  index?: number;
}
export interface LocalDraft {
  source: string;
  draft: string;
  notes?: (JsonValue | null)[];
  acknowledgedNotes?: (JsonValue | null)[];
  updatedAt?: string | null;
  noteForm?: NoteFormDraft;
}
type DraftStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;
export function readLocalDraft(
  storage: DraftStorage,
  key: string,
): LocalDraft | undefined {
  try {
    return JSON.parse(storage.getItem(key) ?? "null") ?? undefined;
  } catch {
    return;
  }
}
export function writeLocalDraft(
  storage: DraftStorage,
  key: string,
  draft: LocalDraft,
) {
  storage.setItem(key, JSON.stringify(draft));
}
export function removeLocalDraft(storage: DraftStorage, key: string) {
  storage.removeItem(key);
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
/**
 * A form editing an existing note keeps its index only while that slot still
 * holds the same note; otherwise it comes back as a new note.
 */
export function reconcileNoteForm(
  form: NoteFormDraft | undefined,
  notes: (JsonValue | null)[],
): NoteFormDraft | undefined {
  if (!form || form.index === undefined) return form;
  const slot = notes[form.index] as Partial<PageNote> | null | undefined;
  const same =
    !!slot &&
    slot.id === form.note.id &&
    slot.createdBy === form.note.createdBy &&
    slot.createdAt === form.note.createdAt;
  return same ? form : { note: form.note };
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
  const notes = unsentNotes ? localNotes! : serverNotes;
  return {
    // An unfinished form has never been sent, even if the server draft is newer.
    noteForm: reconcileNoteForm(local?.noteForm, notes),
    source: local && unsavedLocal ? local.source : (page.tempText ?? page.text),
    notes,
    acknowledgedNotes: JSON.parse(
      JSON.stringify(unsentNotes ? (localAck ?? serverNotes) : serverNotes),
    ) as (JsonValue | null)[],
    unsavedLocal: Boolean(unsavedLocal),
  };
}
