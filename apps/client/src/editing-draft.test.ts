import { expect, test } from "bun:test";
import { restoreDraft } from "./editing-draft";
import type { Page } from "@honkoku/client-api/types";
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
  const fullySent = restoreDraft(page, {
    source: "下書き",
    draft: "下書き",
    notes: sent,
    acknowledgedNotes: sent,
    updatedAt: page.updatedAt,
  });
  expect(fullySent.notes).toEqual(sent);
  expect(fullySent.acknowledgedNotes).toEqual(sent);
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
