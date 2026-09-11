# Backend contract: the editing write protocol

How the website edits a page, as observed in a signed-in capture of one editing cycle on 2026-09-10 (`fixtures/api/edit-cycle-commits.json` holds the four commit requests and their responses) and in the site's bundled client code. The desktop client reproduces these writes byte for byte where they matter, because server-side counters and the activity feed depend on them.

All writes go to the Firestore REST API at `https://firestore.googleapis.com/v1/projects/honkoku3-c466c/databases/(default)/documents` with a bearer ID token. The site's SDK runs every step as an optimistic transaction: a `documents:batchGet` of the documents it will read, then one `documents:commit` whose writes carry `currentDocument.updateTime` preconditions equal to the `updateTime` values just read (plus `verify` writes for documents that were read but not changed). A precondition failure means another client wrote in between; the site retries the whole transaction.

Document ids: a page is `transcriptions/{entryId}_{index}` with `index` zero-based.

## 1. Lock (start editing)

Reads: the page, then `projects/{page.projectId}` when the page carries `projectId`.

Checks: the page exists; the project's `blockedUsers` does not contain the user; `page.status != "editing"`.

Write: `update` on the page with `updateMask` = `prevStatus, status, syncMode, tempEditedBy, tempNotes, tempText, tempTextChanged` and `updateTransforms` = `updatedAt: REQUEST_TIME`:

| field | value |
| --- | --- |
| status | `"editing"` |
| prevStatus | the page's current `status` (`"default"` on a page never edited) |
| tempText | the page's current `text` |
| tempTextChanged | `false` |
| tempNotes | the page's current `notes`, or `[]` |
| tempEditedBy | the user's uid |
| syncMode | the caller's choice, `false` by default |

Plus a `verify` write on the project with its read `updateTime`.

## 2. Draft (autosave while editing)

The site sends this every three seconds while the editor is open. Reads the page; requires `status == "editing"`. Write: `update` with `updateMask` = `tempText, tempTextChanged` (values: the current editor text and `true`) and `updateTransforms` = `updatedAt: REQUEST_TIME`, precondition on the read `updateTime`.

The server's answer carries the new `updateTime`, which becomes the precondition for the next write.

The desktop draft writer includes `tempNotes` in this same commit so text and note changes travel together. A text-only caller copies `tempNotes` from the freshly read page.

### Note transactions

The bundle's `Myn` reads the page, requires `status == "editing"`, and updates only `tempNotes` with the supplied array. `jyn` reads the current `tempNotes` array, replaces the selected index with `null` when it is in bounds, and writes the array back. Neither transaction transforms `updatedAt`. The desktop additionally checks that `tempEditedBy` matches the signed-in account and uses the read `updateTime` precondition.

A note stores `id` (empty string for a new note), `type` (`note`, `memo`, `transcription`, or `other`), `content`, `markdown` (the same text), `createdBy`, `createdAt`, and `updatedAt`. Both dates are Firestore timestamps created on the client. Updates preserve the original ID, author, and creation date. Optional `image` stores the upstream IIIF URL `service/x,y,w,h/300,/0/default.jpg`; optional `xywh` stores `[x,y,w,h]` in rounded full-image pixels. New notes append to the array; presentation order does not change stored indices.

## 3. Save

Reads the page and `projects/{projectId}`. Requires `status == "editing"`. One commit with three writes:

1. `update` on the page, `updateMask` = `editedBy, notes, requestReview, share, status, syncMode, text`, no transforms (so `updatedAt` keeps the value of the last draft write), precondition on the read `updateTime`:

   | field | value |
   | --- | --- |
   | status | the requested status, `"initiated"` unless the caller passes `"completed"` |
   | text | the page's `tempText` (the server's copy, not the local editor) |
   | notes | the page's `tempNotes`, or `[]` |
   | editedBy | the user's uid |
   | share | the caller's choice |
   | requestReview | the caller's choice |
   | syncMode | `false` |
   | approvedBy | only when approving: the page's `approvedBy` with the user appended (at most two reviewers; a third throws); when withdrawing approval: the array without the user |

   `tempText`, `tempNotes`, `tempEditedBy`, and `prevStatus` are left as they are.

2. A new `timelineEvents/{autoId}` document (client-generated 20-character id, plain `update` without precondition, `updateTransforms` = `createdAt: REQUEST_TIME`):

   | field | value |
   | --- | --- |
   | uid, projectId, entryId, transcriptionId, index | identifiers; `eventType` is `"transcription"` |
   | data | the page document as read, with `text` replaced by `tempText`, `editedBy` by the uid, and `status` by the saved status |
   | members, projectType | copied from the project document |
   | count | the number of non-whitespace characters added between the old `text` and `tempText`, from a character-level diff (jsdiff `diffChars`): the sum of `value.replace(/\s/g, "").length` over the added hunks |
   | isReview | `true` when the page had `requestReview == true` and its `editedBy` is a different user |
   | status, share, requestReview, comment, isApproval | the caller's options, spread as given (`comment` is `""` when empty) |

3. `verify` on the project with its read `updateTime`.

Server-side triggers, invisible to the client, update the user's `exp`, `level`, and `charCount`, the project's counters, and notifications.

## 4. Discard

Reads the page; requires `status == "editing"`. Write: `update` with `updateMask` = `status` set to the page's `prevStatus`. Nothing else changes; the draft fields stay on the document.

## 5. OCR result

`update` on the page with `updateMask` = `ocr.{engine}` (`ndl` or `minna`) and `updatedAt: REQUEST_TIME`, precondition on the read `updateTime`. The value is the engine's result object.

## Observed timing and shapes

- The captured cycle: lock at t+0, drafts at t+19 s and t+46 s, save at t+52 s, each commit answered in about 150 ms with `writeResults[].updateTime` and `transformResults[].timestampValue`.
- `updateTime` carries nanosecond precision (`2026-09-10T07:41:41.693887000Z`); preconditions must round-trip it unchanged.
- The `verify` write on the project makes a concurrent project edit (member changes, blocking) fail the save, which is the behaviour to keep.
- No lock lease exists. A page left in `editing` by a crashed client stays locked until that user discards or saves.
