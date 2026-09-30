import { expect, test } from "bun:test";
import {
  notesPending,
  restoreDraft,
  reconcileNoteForm,
  readLocalDraft,
  writeLocalDraft,
  removeLocalDraft,
  type LocalDraft,
  type NoteFormDraft,
} from "./editing-draft";
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

function draftStorage() {
  const records = new Map<string, string>();
  return {
    getItem: (key: string) => records.get(key) ?? null,
    setItem: (key: string, value: string) => {
      records.set(key, value);
    },
    removeItem: (key: string) => {
      records.delete(key);
    },
  };
}
const noteForm: NoteFormDraft = {
  note: {
    content: "入力途中の注釈",
    type: "memo",
    image: "https://example.org/iiif/10,20,30,40/300,/0/default.jpg",
    xywh: [10, 20, 30, 40],
  },
};
const localWithForm: LocalDraft = {
  source: "未送信の本文",
  draft: page.tempText!,
  notes: [{ content: "追加済みの注釈" }],
  acknowledgedNotes: page.tempNotes!,
  updatedAt: page.updatedAt,
  noteForm,
};
test("a note form survives storage round trips independently for each page", () => {
  const storage = draftStorage();
  const first = "honkoku.edit.user.entry.0";
  const second = "honkoku.edit.user.entry.1";
  writeLocalDraft(storage, first, localWithForm);
  writeLocalDraft(storage, second, {
    ...localWithForm,
    noteForm: {
      index: 0,
      note: { content: "既存注釈の編集途中", type: "other" },
    },
  });
  const restored = restoreDraft(page, readLocalDraft(storage, first));
  expect(restored.noteForm).toEqual(noteForm);
  expect(restored.source).toBe(localWithForm.source);
  expect(restored.notes).toEqual(localWithForm.notes!);
  expect(restored.acknowledgedNotes).toEqual(localWithForm.acknowledgedNotes!);
  expect(readLocalDraft(storage, second)?.noteForm).toEqual({
    index: 0,
    note: { content: "既存注釈の編集途中", type: "other" },
  });
  expect(readLocalDraft(storage, "honkoku.edit.other.entry.0")).toBeUndefined();
});
test("a newer server draft supersedes text without dropping the unsent form", () => {
  const result = restoreDraft(page, {
    ...localWithForm,
    updatedAt: "2026-09-10T07:41:41.693886000Z",
  });
  expect(result.source).toBe(page.tempText!);
  expect(result.notes).toEqual(page.tempNotes!);
  expect(result.noteForm).toEqual(noteForm);
});
test("an unsent form survives a missing local timestamp and empty content", () => {
  const storage = draftStorage();
  const emptyForm = { note: { ...noteForm.note, content: "" } };
  writeLocalDraft(storage, "page", {
    ...localWithForm,
    updatedAt: undefined,
    noteForm: emptyForm,
  });
  expect(restoreDraft(page, readLocalDraft(storage, "page")).noteForm).toEqual(
    emptyForm,
  );
});
test("clearing an added or cancelled form preserves the page draft", () => {
  const storage = draftStorage();
  writeLocalDraft(storage, "page", localWithForm);
  writeLocalDraft(storage, "page", {
    ...readLocalDraft(storage, "page")!,
    noteForm: undefined,
  });
  const stored = readLocalDraft(storage, "page");
  expect(stored?.noteForm).toBeUndefined();
  expect(stored?.source).toBe(localWithForm.source);
  expect(stored?.notes).toEqual(localWithForm.notes!);
});
test("discard removes the form with its page draft, leaving other pages intact", () => {
  const storage = draftStorage();
  writeLocalDraft(storage, "first", localWithForm);
  writeLocalDraft(storage, "second", localWithForm);
  removeLocalDraft(storage, "first");
  expect(readLocalDraft(storage, "first")).toBeUndefined();
  expect(
    restoreDraft(page, readLocalDraft(storage, "first")).noteForm,
  ).toBeUndefined();
  expect(readLocalDraft(storage, "second")?.noteForm).toEqual(noteForm);
});
test("legacy and unreadable records restore without a form", () => {
  const storage = draftStorage();
  writeLocalDraft(storage, "legacy", { source: "本文", draft: "本文" });
  storage.setItem("broken", "{");
  expect(
    restoreDraft(page, readLocalDraft(storage, "legacy")).noteForm,
  ).toBeUndefined();
  expect(readLocalDraft(storage, "broken")).toBeUndefined();
  expect(readLocalDraft(storage, "missing")).toBeUndefined();
});
test("storage write failures are surfaced so the workbench can warn the user", () => {
  const storage = {
    ...draftStorage(),
    setItem: () => {
      throw Error("quota");
    },
  };
  expect(() => writeLocalDraft(storage, "page", localWithForm)).toThrow(
    "quota",
  );
});
test("a restored note form keeps its index only while the slot holds the same note", () => {
  const mine = {
    id: "",
    content: "A",
    createdBy: "u",
    createdAt: "2026-09-29T00:00:00Z",
  };
  const other = {
    id: "",
    content: "B",
    createdBy: "u",
    createdAt: "2026-09-29T01:00:00Z",
  };
  const form = { note: { ...mine, content: "A revision" }, index: 0 };
  expect(reconcileNoteForm(form, [mine])).toEqual(form);
  expect(reconcileNoteForm(form, [other])).toEqual({ note: form.note });
  expect(reconcileNoteForm(form, [])).toEqual({ note: form.note });
  expect(reconcileNoteForm({ note: form.note }, [other])).toEqual({
    note: form.note,
  });
});
