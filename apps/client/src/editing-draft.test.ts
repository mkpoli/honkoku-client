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
});
test("missing or invalid local timestamps resume server text", () => {
  for (const updatedAt of [undefined, null, "invalid"])
    expect(
      restoreDraft(page, { source: "旧本文", draft: "", updatedAt }).source,
    ).toBe(page.tempText!);
  expect(restoreDraft(page).notes).toEqual(page.tempNotes!);
});
