import { expect, test } from "bun:test";
import { notesPending, restoreDraft } from "./editing-draft";
import type { JsonValue, Page } from "@honkoku/client-api/types";
const page: Page = {
  id: "entry_0",
  entryId: "entry",
  index: 0,
  status: "editing",
  text: "本文",
  tempText: "サーバーの下書き",
  notes: [],
  tempNotes: [{ content: "新しい注記" }],
  updatedAt: "2026-09-10T07:41:41.693887000Z",
};
test("a newer server draft supersedes older local text and notes", () => {
  const result = restoreDraft(page, {
    source: "古い未送信の下書き",
    draft: "旧本文",
    notes: [],
    updatedAt: "2026-09-10T07:41:41.693886000Z",
  });
  expect(result.source).toBe(page.tempText!);
  expect(result.notes).toEqual(page.tempNotes!);
  expect(result.acknowledgedNotes).toEqual(page.tempNotes!);
});
test("a current local draft keeps text not yet sent", () => {
  const result = restoreDraft(page, {
    source: "未送信の下書き",
    draft: page.tempText!,
    notes: [null],
    updatedAt: page.updatedAt,
  });
  expect(result.source).toBe("未送信の下書き");
  expect(result.notes).toEqual([null]);
  expect(result.acknowledgedNotes).toEqual(page.tempNotes!);
  expect(result.unsavedLocal).toBe(true);
});
test("acknowledged notes separate sent from unsent local edits", () => {
  const sent = [{ content: "送信済み" }];
  const result = restoreDraft(page, {
    source: "下書き",
    draft: "下書き",
    notes: [{ content: "未送信" }],
    acknowledgedNotes: sent,
    updatedAt: page.updatedAt,
  });
  expect(result.notes).toEqual([{ content: "未送信" }]);
  expect(result.acknowledgedNotes).toEqual(sent);
  expect(notesPending(result.notes, result.acknowledgedNotes)).toBe(true);
});
test("a fully sent local draft yields to the server's notes", () => {
  const sent = [{ content: "送信済み" }];
  const result = restoreDraft(page, {
    source: "下書き",
    draft: "下書き",
    notes: sent,
    acknowledgedNotes: sent,
    updatedAt: page.updatedAt,
  });
  expect(result.notes).toEqual(page.tempNotes!);
  expect(result.acknowledgedNotes).toEqual(page.tempNotes!);
  expect(notesPending(result.notes, result.acknowledgedNotes)).toBe(false);
});
test("untouched notes do not read as a local change", () => {
  const notes: (JsonValue | null)[] = [
    { content: "書入れ", createdAt: "2026-09-10T01:00:00.100Z" },
  ];
  const acknowledged = JSON.parse(JSON.stringify(notes));
  expect(notesPending(notes, acknowledged)).toBe(false);
  const edited = JSON.parse(JSON.stringify(notes));
  edited[0].content = "改めた書入れ";
  expect(notesPending(edited, acknowledged)).toBe(true);
});
test("the gate reads the sent payload, not a reshaped echo", () => {
  const notes: (JsonValue | null)[] = [
    { id: "", content: "x", createdAt: "2026-09-10T01:00:00.100Z" },
  ];
  const acknowledged = JSON.parse(JSON.stringify(notes));
  const echo = [
    { createdAt: "2026-09-10T01:00:00.1Z", content: "x", id: "" },
  ] as (JsonValue | null)[];
  expect(notesPending(notes, echo)).toBe(true);
  expect(notesPending(notes, acknowledged)).toBe(false);
});
test("missing or invalid local timestamps resume server text", () => {
  for (const updatedAt of [undefined, null, "invalid"]) {
    const result = restoreDraft(page, {
      source: "旧本文",
      draft: "",
      updatedAt,
    });
    expect(result.source).toBe(page.tempText!);
    expect(result.unsavedLocal).toBe(false);
  }
  expect(restoreDraft(page).notes).toEqual(page.tempNotes!);
  expect(restoreDraft(page).acknowledgedNotes).toEqual(page.tempNotes!);
});
test("a leftover local record without unsaved text does not win", () => {
  const result = restoreDraft(page, {
    source: "",
    draft: "",
    updatedAt: page.updatedAt,
  });
  expect(result.source).toBe(page.tempText!);
  expect(result.unsavedLocal).toBe(false);
});
