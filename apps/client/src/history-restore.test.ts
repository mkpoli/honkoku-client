import { expect, test } from "bun:test";
import { historyVersion, restoreHistoryDraft } from "./history-restore";
import type { JsonValue } from "@honkoku/client-api/types";

const currentNotes = [{ content: "現在の注記" }];

test("restore uses the saved version's tempNotes instead of the previous notes", () => {
  const data = {
    text: "復元する本文",
    tempText: "別の本文",
    notes: [{ content: "前の版の注記" }],
    tempNotes: [{ content: "復元する注記" }],
  };
  expect(restoreHistoryDraft(historyVersion(data), currentNotes)).toEqual({
    source: "復元する本文",
    notes: data.tempNotes,
  });
  expect(currentNotes).toEqual([{ content: "現在の注記" }]);
});

test("notes-only snapshots restore their recorded notes", () => {
  for (const tempNotes of [undefined, null]) {
    const data = {
      text: "本文",
      notes: [{ content: "記録された注記" }],
      ...(tempNotes === undefined ? {} : { tempNotes }),
    };
    expect(restoreHistoryDraft(historyVersion(data), currentNotes)).toEqual({
      source: data.text,
      notes: data.notes,
    });
  }
});

test("an explicitly empty notes array clears current notes", () => {
  const snapshots: JsonValue[] = [
    { text: "本文", tempNotes: [], notes: currentNotes },
    { text: "本文", notes: [] },
  ];
  for (const data of snapshots) {
    const version = historyVersion(data);
    expect(version.notes).toEqual([]);
    expect(restoreHistoryDraft(version, currentNotes).notes).toEqual([]);
  }
});

test("a version without recorded notes replaces only text", () => {
  const snapshots: JsonValue[] = [
    { text: "旧本文" },
    { text: "旧本文", notes: null, tempNotes: null },
  ];
  for (const data of snapshots) {
    const version = historyVersion(data);
    expect(version.notes).toBeUndefined();
    const restored = restoreHistoryDraft(version, currentNotes);
    expect(restored.source).toBe("旧本文");
    expect(restored.notes).toBe(currentNotes);
  }
});

test("restored notes keep indices, metadata and regions without mutating history", () => {
  const note = {
    id: "note-1",
    type: "note",
    content: "注記",
    markdown: "注記",
    createdBy: "author",
    createdAt: "2026-09-10T00:00:00Z",
    updatedAt: "2026-09-11T00:00:00Z",
    image: "https://example.com/image.jpg",
    xywh: [1, 2, 30, 40],
  };
  const data = { text: "[注記]", tempNotes: [null, note] };
  const version = historyVersion(data);
  const restored = restoreHistoryDraft(version, currentNotes);
  expect(restored.notes).toEqual([null, note]);
  expect(restored.notes).not.toBe(data.tempNotes);
  const restoredNote = restored.notes[1] as typeof note;
  restoredNote.content = "編集後";
  restoredNote.xywh[0] = 99;
  expect(data.tempNotes[1]).toEqual(note);
  expect(note.content).toBe("注記");
  expect(note.xywh).toEqual([1, 2, 30, 40]);
  expect(restoreHistoryDraft(version, currentNotes).notes).toEqual([null, note]);
});

test("unchanged text still restores notes", () => {
  const version = historyVersion({ text: "本文", tempNotes: [null] });
  expect(restoreHistoryDraft(version, currentNotes)).toEqual({
    source: "本文",
    notes: [null],
  });
});

test("missing or malformed snapshots do not invent recorded notes", () => {
  for (const data of [undefined, null, [], "text", { notes: "invalid" }] as (
    | JsonValue
    | undefined
  )[]) {
    expect(historyVersion(data)).toEqual({ source: "", notes: undefined });
  }
});
