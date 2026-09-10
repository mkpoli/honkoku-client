# Backend contract: facts confirmed by a signed-in capture

Companion to `api-report.md`, whose open questions these observations settle or narrow. Captured on 2026-09-10 with `tools/session/login.ts` from a browser session signed in through X.

## Authentication

- Sign-in with X goes through the Firebase Auth handler on the authDomain, then `identitytoolkit.googleapis.com/v1/accounts:signInWithIdp` and `accounts:lookup`. The resulting ID token has `aud` `honkoku3-c466c`, `sign_in_provider` `twitter.com`, and a one-hour lifetime; the refresh token from the Secure Token API renews it.
- No `X-Firebase-AppCheck` header is sent on any Firestore or Identity Toolkit request. App Check is not enforced, so the REST API with a bearer ID token behaves like the SDK.
- The Firebase SDK reaches Firestore over the WebChannel transport (`google.firestore.v1.Firestore/Listen/channel`). Request bodies are URL-encoded JSON `addTarget` messages whose `structuredQuery` objects are the same shapes the REST `runQuery` endpoint accepts.

## Live queries opened by the home page

| Target | Query |
| --- | --- |
| Announcements | `adminAnnouncements` where `display == true`, order `createdAt` desc, limit 5 |
| Timeline | `timelineEvents` where `eventType == "transcription"` and `projectType in ["official","user"]` and `share == true`, order `createdAt` desc, limit 20 |
| Site progress | `dailyProgress` order `timestamp` desc, limit 2 (today and yesterday, for the deltas) |
| Ranking | `users` order `exp` desc, limit 100 |
| Own profile | document `users/{uid}` |
| Feed enrichment | one document listener each for the `entries/{id}`, `projects/{id}`, `users/{uid}`, and `likes/timelineEvents_{eventId}` referenced by timeline rows |

Unread notifications are counted with `runAggregationQuery` (count) on `notifications` where `uid == {uid}`.

## Reads with a bearer token

- `users/{uid}` is readable with and without a token. Beyond the fields in the report it carries `updatedAt` and `dataSucceeded` (boolean; set on accounts migrated from V2).
- `emailSettings/{uid}` answers NOT_FOUND for an account that never saved settings.
- `notifications` aggregate count works for the signed-in user's own `uid`.
- The search service at `search-r7au5bknyq-uc.a.run.app` answers `{page, size, status, total, totalPages, results[]}`; each result is `{id, text, projectId, entryId, index, createdAt (epoch ms), highlight[]}` with `<em>` around matches in `highlight`.

## Shapes seen in harvested page documents

- `notes[]` elements: `{id, type: "note", content, markdown, createdBy (uid), createdAt, updatedAt}`; `id` is an empty string in every sample, so position in the array is the identity, which matches the null-out deletion the report describes.
- `translations` on a page: `{translationJa, translationEn, annotationsJa, annotationsEn}`, markdown strings produced by the site's modern-translation feature.
- `annotations[]` was empty in every harvested page of the ainu project; its element shape remains unknown.

## Still open

- Bodies of the editing transaction (lock, draft writes, save, discard) on the `Write/channel` transport; a capture of one editing cycle is needed.
- Whether `default` appears as a literal status on never-touched pages or the document is simply absent.
- Accrual of `stoneCount`.
