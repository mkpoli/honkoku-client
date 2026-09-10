# みんなで翻刻 V3 (app.honkoku.org) — client bundle reverse-engineering report

Source: `app-bundle.js` (5.1MB, minified Vite/esbuild build), `app-bundle.css`.
All minified identifiers referenced below (e.g. `rt`, `Xt`, `Sn`, `Bn`, `em`) are
stable *within this one build* only; they will change on the next deploy. They
are given here so a reader can `rg` the same bundle and verify every claim.

Firestore/Firebase SDK helper aliases identified by usage pattern (used throughout this report):

| alias | Firebase API |
|---|---|
| `H7` | `initializeApp()` |
| `em` | `getAuth(H7)` — the Auth instance |
| `rt` | `getFirestore(H7)` — the Firestore instance |
| `A2` | `getFunctions(H7)` |
| `V7` | `getStorage()` |
| `Xt(rt,"col","id")` | `doc()` |
| `Sn(rt,"col")` | `collection()` |
| `Bn(q,...)` | `query()` |
| `_n(field,op,val)` | `where()` |
| `di(field,dir)` | `orderBy()` |
| `Su(n)` | `limit()` |
| `zr()` | `serverTimestamp()` |
| `ZT(n)` | `increment(n)` |
| `qo(rt, fn)` | `runTransaction()` |
| `T2(ref,data[,opts])` | `setDoc()` |
| `Qs(ref,data)` | `updateDoc()` |
| `vm(colRef,data)` | `addDoc()` |
| `ku(ref)` | `getDoc()` |
| `eu(query)` | `getDocs()` |
| `kse(query)` | `getCountFromServer()` |
| `Zp(ref)` | `deleteDoc()` |
| `iN(ref/query,cb,errCb)` | `onSnapshot()` |
| `wn(ref,deps)` | app-level React hook: subscribes one doc via `onSnapshot`, returns `{data,loading,error}` |
| `Zr(query,deps)` | app-level React hook: subscribes a query via `onSnapshot`, returns `{data,loading,error}` |
| `k2(A2,"name")` | `httpsCallable(functions,"name")` |
| `Mse(storage,path)` | `ref()` (Storage) |
| `Nse(ref,file)` | `uploadBytes()` |
| `Lse(ref)` | `getDownloadURL()` |
| `cg` | `class extends … { constructor(){super("google.com")…} }` = `GoogleAuthProvider` |
| `dg` | same for `"twitter.com"` = `TwitterAuthProvider` |
| `Cve(auth,provider)` | `signInWithPopup()` |
| `APt(auth,cb)` | `onAuthStateChanged()` |
| `ide` | `diff` (jsdiff) word/char diff, used to count changed characters |

---

## 1. Stack

- **UI framework**: React 18 (`createRoot` is used; `React.Component` class components with `getDerivedStateFromError` also present for an error boundary).
- **Component library**: Material UI (`@mui/material` — recognizable `muiName` static-property pattern on icon components, `sx` prop usage throughout, `Dialog`/`DialogTitle`/`DialogContent`/`Snackbar`/`Chip`/`Table` components everywhere). No exact semver string found in the bundle.
- **Router**: React Router v6, built on `@remix-run/router` — the literal banner is present:
  ```
  /**
   * @remix-run/router v1.23.2
   * Copyright (c) Remix Software Inc.
   */
  ```
  The route tree is built with `createBrowserRouter`-equivalent (minified to `sMt(...)`) from a single nested array. Full route table reconstructed from the literal `{path:"…",element:m.jsx(Component,{})}` array (`VWn=sMt([...])`):

  ```
  / (root layout, wraps auth bootstrap "gHt")
  ├─ /                                              home feed
  ├─ /timeline                                      global transcription timeline
  ├─ /recentTranscriptions
  ├─ /videos                                        tutorial video list (YouTube embeds)
  ├─ /forum                                          global forum
  │  ├─ /forum/new
  │  ├─ /forum/:postId
  │  └─ /forum/:postId/edit
  ├─ /projects/:type?                                project list; children "official"|"user"|"private" (tab state, no own element)
  ├─ /newProject
  ├─ /projects/:id                                   project detail, children:
  │  ├─ info | guidelines
  │  ├─ forum | forum/new | forum/:postId | forum/:postId/edit
  │  ├─ collections/:collectionId
  │  ├─ timeline
  │  └─ announcements
  ├─ /search                                         global search
  ├─ /translations                                   "proofread" (校正/翻訳) module, children:
  │  ├─ (index → redirect to entries)
  │  ├─ entries | entries/:project
  │  ├─ timeline
  │  ├─ forum | forum/new | forum/:postId | forum/:postId/edit
  │  └─ * → redirect to entries
  ├─ /imageCollection | /imageCollection/:id | /users/:uid/imageCollection
  ├─ /notifications
  ├─ /messages | /messages/:conversationId
  ├─ /users/:uid                                     public profile
  ├─ /editingHistory                                 "作業履歴" — current user's own edit history
  ├─ /AdminAnnouncement | /AdminAnnouncements         site-wide announcements admin
  ├─ /admin | /admin/users                            site admin dashboard / user search
  ├─ /adminProject/:id                                per-project admin panel, children:
  │  info, guidelines, entries, progress, members, blocked, requests, announcements, functions
  ├─ /transcription/:entryId/:page                    the page-transcription editor, children (all rendered inside the same component via nested `<Outlet>`, no separate element):
  │  index, input, ocr, history, notes, notes/:index/view, notes/:index/edit, notes/new, translation
  └─ /translations/entries/:project/:entryId          per-entry translation/proofreading editor
  ```
  Evidence (excerpt): `{path:"/adminProject/:id",element:m.jsx(LWn,{}),children:[{path:"info",...},{path:"guidelines",...},{path:"entries",element:m.jsx(Vyn,{})},{path:"progress",...},{path:"members",...},{path:"blocked",...},{path:"requests",...},{path:"announcements",...},{path:"functions",...}]}`

- **State management**: **Jotai** (`globalThis.__JOTAI_DEFAULT_STORE__`, `Symbol.for("JOTAI.EXPERIMENTAL.FLUSHSTOREHOOK")`). Atoms are created with a minified `ns(...)` and read/write with `$n(atom)`/`Cr(atom)`. All server data, by contrast, is *not* held in global state — it is fetched ad hoc per-component through the `wn`/`Zr` onSnapshot hooks (see §4). Redux + react-redux code is also bundled (`@@redux/INIT`, `react-redux-context`, `reduxStoreName`) but only ever appears next to the rich-text editor plumbing — almost certainly an internal dependency of the MDXEditor package, not the app's own state store.
- **Editor library for transcription text**: **CodeMirror 6** (`@codemirror/state`/`view` — the distinctive `"CodeMirror plugin crashed"` error string and `ViewPlugin`-shaped classes are present). It is used as a plain-text line editor for the transcription body; the special notation (see §8) is plain Unicode/ASCII text typed into CodeMirror, not implemented as syntax-highlighted CodeMirror decorations as far as could be confirmed — no CodeMirror `Decoration`/`StateField` code specific to the markup grammar was found; instead there is a **separate hand/ANTLR4-generated parser** (`pRe.CommonToken`, `tokenSource.nextToken()` — classic antlr4 runtime symbols) that parses the saved text into an AST for preview/HTML/Word/LaTeX/XML rendering (see §8).
- **Rich markdown editor**: **MDXEditor** (`@mdxeditor/editor`-shaped API — `uk({toolbarContents…})`, `J2()`,`nk()`,`K2()`,`ck()`,`X2()`,`aU()`,`RU()`,`hU()`,`Uqe({defaultCodeBlockLanguage:"javascript"})`,`MXe({codeBlockLanguages:{...}})`) is used for the **project markdown body** (`admin.projectInfo.markdown`) and **project guidelines** (`admin.guidelines`) fields — full WYSIWYG markdown with tables, code blocks, headings, links.
- **WASM/ONNX in-browser OCR**: yes — **onnxruntime-web** (`onnxruntime-web@1.26.0`, loaded from `https://cdn.jsdelivr.net/npm/onnxruntime-web@1.26.0/dist/`) runs a **ConvNeXt** kuzushiji-recognition model client-side, entirely in the browser:
  ```
  rVn="https://huggingface.co/yuta1984/soramaru_kuzushiji_ai/resolve/main/convnext_v4.onnx",
  iVn="/model/convnext_v4.meta.json", oVn="/model/unicode_translation.csv",
  sVn="soramaru-kuzushiji"
  ```
  The `.meta.json` and `unicode_translation.csv` are same-origin (`fetch(iVn)`/`fetch(oVn)`), the `.onnx` weights are fetched directly from HuggingFace by the browser. This is the "みんなで翻刻OCR" (`ocr.engine.minna`) engine. UI copy: `"ocr.initialization.warning":"初回実行時には、モデルのダウンロードのため初期化に時間がかかります。"` (first run is slow because the model has to download).
- **i18n**: `i18next` + `react-i18next` (`useTranslation`→`pt()`) + `i18next-browser-languagedetector` (`_ct.type="languageDetector"`). Both `ja` and `en` resource catalogs (≈1770 keys total) are inlined directly in the bundle as one big object literal (`R4e={ja:{...},en:{...}}`), i.e. not lazily chunk-split. Separately, longer help/guideline prose is fetched at runtime from same-origin static files: `fetch(`/locales/markdowns/${key}`)`.
- **Image viewer**: **OpenSeadragon**, driving IIIF Image API tile sources (`http://iiif.io/api/image/2|3/context.json` compliance negotiation code, CSS classes `openseadragon-canvas`, `openseadragon-container`, `openseadragon-overlay-`). Manifests are IIIF Presentation API manifests, imported server-side via the `importIiifManifest` callable (see §6).
- **Export**: JSZip is bundled (`ZipFileWorker`, `zipComment`) for bulk-downloading multiple export files at once; single small downloads use a `data:` URI `<a download>` trick. The markup AST carries per-element `textTemplate` / `htmlTemplate` / `latexTemplate` / `wordTemplate` (literal OOXML `<w:t>` fragments) / `xmlTemplate` — i.e. transcriptions can be rendered/exported to plain text, HTML, LaTeX and Word-XML (and at least TEI-shaped XML for some elements), and the "proofread"/translation module offers a `"Download TSV"` export.
- **Other notable libraries**: `axios` (only used for the external Metom OCR call, see §6), `date-fns` (+ `date-fns-tz` locale tables), `jsdiff` (word/char diffing for scoring), `@remix-run/router`.

---

## 2. Auth

- **Providers**: Only **Google** and **Twitter/X**, both via **popup** sign-in (`signInWithPopup`). No password auth, no email-link auth, no anonymous auth, no Facebook/GitHub — those provider classes exist in the bundle (`PROVIDER_ID="facebook.com"`, `"github.com"`) purely because they ship inside the Firebase Auth SDK; they are never instantiated by app code (`rg -o "new cg\("`/`"new dg\("` — the *only* two provider classes actually `new`'d in the app).

  Login dialog, in full:
  ```js
  const THt=()=>{const[t,e]=$n(Wse),n=()=>{e(!1)},
    r=async()=>{try{const o=new cg;await Cve(em,o),n()}catch(o){console.error("Google login error:",o)}},
    i=async()=>{try{const o=new dg;await Cve(em,o),n()}catch(o){console.error("Twitter login error:",o)}};
    return m.jsxs(sr,{open:t,onClose:n,children:[m.jsx(br,{children:"ログイン"}),
      m.jsx(ar,{children:m.jsxs(ji,{spacing:2,sx:{mt:1},children:[
        m.jsx(Qe,{...,onClick:r,children:"Googleでログイン"}),
        m.jsx(Qe,{...,onClick:i,children:"Xでログイン"})]})})]})}
  ```

- **ID-token attachment for REST calls**: every hand-written `fetch()` to a Cloud Run/Cloud Functions HTTP endpoint (see §3) does the same thing —
  ```js
  const n={"Content-Type":"application/json"}, r=em.currentUser;
  if(r) try{ const s=await r.getIdToken(); n.Authorization=`Bearer ${s}` }catch(s){ console.warn("Failed to get auth token:",s) }
  ```
  i.e. header `Authorization: Bearer <Firebase-ID-token>`, added **only if a user is signed in** and **silently omitted on failure** — meaning these endpoints must tolerate anonymous calls (the calling code never blocks the request if `getIdToken()` fails). The one exception is the `imageproxy` Cloud Run service, which requires a signed-in user (throws `"User not authenticated"` client-side before even calling `getIdToken()` unconditionally, i.e. it hard-requires auth):
  ```js
  const r=Mje().currentUser; if(!r) throw new Error("User not authenticated");
  const i=await r.getIdToken(), o=`https://imageproxy-r7au5bknyq-uc.a.run.app?url=${encodeURIComponent(t)}`,
  s=await fetch(o,{method:"GET",headers:{Authorization:`Bearer ${i}`}});
  ```
- **No custom-token / session-cookie flow** was found anywhere — auth is 100% client-SDK-managed Firebase Auth, ID tokens minted on demand per request.
- **`users/{uid}` doc, written on first sign-in** by the `onAuthStateChanged` listener if the doc does not already exist:
  ```js
  const s={uid:r.uid,displayName:r.displayName||"Anonymous",profile:"",website:"",
           photoURL:r.photoURL||"",level:1,exp:0,likeCount:0,charCount:0,stoneCount:0,
           createdAt:zr()};
  await T2(i,s);
  ```
  Profile-settings dialog additionally lets the user edit `displayName` and `profile` directly via `updateDoc` (client write, no validation beyond what Firestore security rules presumably enforce). `website` is in the initial doc but no UI to edit it was found (dead field, or edited elsewhere not reached).
- **Email settings** are read directly from Firestore (`emailSettings/{uid}`) but only *written* through a callable Cloud Function `updateEmailSettings` (not a direct client write) — see §6.

---

## 3. REST API

**Important finding:** the SPA itself makes almost no calls to the previously-known `https://app.honkoku.org/api/*` Express endpoints (`/api/projects`, `/api/projects/{id}`, `/api/collections/{id}`, `/api/entries/{id}`) — an exhaustive `rg -o '/api/[a-zA-Z0-9/_-]*'` over the whole bundle turns up only `/api/image` and `/api/predict`, both of which are unrelated (`/api/image/...context.json` are literal **IIIF Image API compliance-level URLs** hard-coded in the OpenSeadragon library, not calls the app makes; `/api/predict` is the tail of the external Metom endpoint below). **The public `/api/projects` etc. Express service therefore appears to be a separate consumer-facing API that the bundle itself does not call — see §11.** Instead, almost all data access is direct Firestore SDK reads/writes (§4), and the handful of REST calls the SPA *does* make all go to bespoke Cloud Functions / Cloud Run services with hand-written `fetch()`/`axios` calls. All of these were found and are listed below, grouped by resource.

### Search

- **`GET https://search-r7au5bknyq-uc.a.run.app?keyword=&projectIds=&entryId=&page=&size=`** — full-text transcription search. No path param; query params `keyword`, `projectIds` (singular `projectId` param name maps to `projectIds` query key), `entryId`, `page`, `size`. Auth optional (Bearer attached if signed in).
  ```js
  async function hKt(t){const e=new URLSearchParams;t.keyword&&e.append("keyword",t.keyword),
    t.projectId&&e.append("projectIds",t.projectId),t.entryId&&e.append("entryId",t.entryId),
    t.page!==void 0&&e.append("page",String(t.page)),e.append("size",String(t.size));
    ...const i=await fetch(`${nze}?${e.toString()}`,{method:"GET",headers:n});
    if(!i.ok)throw new Error(`Failed to fetch transcriptions: ${i.status}`);return await i.json()}
  ```
- **`GET https://search-r7au5bknyq-uc.a.run.app/users?keyword=&sortField=&sortOrder=&page=&size=`** — user search (used by admin user search / ranking pages). Auth optional.
  ```js
  async function CN(t){...const i=await fetch(`${nze}/users?${e.toString()}`,{method:"GET",headers:n});
    if(!i.ok)throw new Error(`Failed to fetch users: ${i.status}`);return await i.json()}
  ```
- **`GET https://us-central1-honkoku3-c466c.cloudfunctions.net/search/image-collection-entries?uid=&keyword=&includeOthers=&page=&size=`** — search over the personal "image collection" bookmarks. Auth optional.
  ```js
  const TWn="https://us-central1-honkoku3-c466c.cloudfunctions.net/search";
  ...const i=await fetch(`${TWn}/image-collection-entries?${e.toString()}`,{method:"GET",headers:n});
  ```
  **Inferred**: `search-r7au5bknyq-uc.a.run.app` and `us-central1-honkoku3-c466c.cloudfunctions.net/search` are very likely the *same* underlying 2nd-gen Cloud Function ("search") — Cloud Functions (2nd gen) always deploy as a Cloud Run service, exposing both a `cloudfunctions.net` alias and an autogenerated `*-uc.a.run.app` hostname for the identical backend. If so, the one Express-style service exposes at least three routes: `GET /` (transcriptions), `GET /users`, `GET /image-collection-entries`. Not confirmed by direct network capture.

### Image proxy

- **`GET https://imageproxy-r7au5bknyq-uc.a.run.app?url={encoded IIIF/image URL}`** — CORS-safe proxy for fetching an arbitrary remote image (used before drawing onto a `<canvas>`, e.g. for OCR input or cropping a "image collection" snippet). **Requires auth** (throws client-side if no `currentUser`).
  ```js
  const Fot=async(t,e)=>{const r=Mje().currentUser;if(!r)throw new Error("User not authenticated");
    const i=await r.getIdToken(),o=`https://imageproxy-r7au5bknyq-uc.a.run.app?url=${encodeURIComponent(t)}`,
    s=await fetch(o,{method:"GET",headers:{Authorization:`Bearer ${i}`}});
    if(!s.ok)throw new Error(`Proxy error! status: ${s.status}`);...}
  ```

### External third-party OCR (not honkoku's own backend, but called directly from the browser)

- **`POST https://mp.ex.nii.ac.jp/metom/api/predict`** — NII's "Metom" single-character kuzushiji-recognition API. No auth header sent (public third-party endpoint). Body: `{image_base64, k:10, return_probs:true}` where `image_base64` is a cropped character image encoded client-side via `<canvas>.toDataURL`.
  ```js
  class _Oe{static async recognizeWithMetom(e){
    const n="https://mp.ex.nii.ac.jp/metom/api/predict",r=await this.convertImageToBase64(e),
    i=await Ss.post(n,{image_base64:r.base64,k:10,return_probs:!0},
      {headers:{"Content-Type":"application/json"},timeout:3e4});
    return{predictions:i.data.predictions||[]}}}
  ```
  Response `predictions` is mapped to `{character, score, rank, reference: https://codh.rois.ac.jp/char-shape/unicode/U+<hex>}` — i.e. each predicted character is also cross-linked to CODH's kuzushiji character-shape database.
- NDL's "古典籍OCR-Lite" is **not called as an API** — the UI only links out to `https://ndlkotenocr-lite-web.netlify.app/` for the user to run OCR themselves in a separate tab; the app has no client-side call to any `ndl*` OCR host. (Existing `ocr.ndl` results *are* read from and written to Firestore — see §4 — presumably populated by some other, non-SPA, process; not confirmed.)

### Firebase platform REST (not honkoku-specific, included for completeness)

Standard Firebase Auth/Identity Toolkit REST paths appear as literal strings inside the bundled `firebase/auth` SDK (used internally by the SDK, not called directly by app code):
`/v1/accounts:delete`, `/v1/accounts:lookup`, `/v1/accounts:signInWithIdp`, `/v1/token`, `/v2/accounts:revokeToken`, `/v2/passwordPolicy`.

### `/api/*` Express service (already known, NOT exercised by this bundle)

`GET /api/projects`, `/api/projects/{id}`, `/api/collections/{id}`, `/api/entries/{id}` — confirmed to exist by the requester independently (curl/headers), but no code path in this bundle calls them. Marked **inferred/unconfirmed from this artifact** — see §11.

---

## 4. Firestore direct access

Nearly every read in the app goes through one of two custom React hooks that wrap `onSnapshot` — i.e. **most reads are live, real-time listeners**, not one-shot fetches:

```js
function wn(t,e=[]){ // subscribe a single DocumentReference
  ...useEffect(()=>{ if(!t)return; return iN(t,l=>{r(l.exists()?{id:l.id,...l.data()}:null),...},
    l=>{console.error("Error fetching document:",l),...}) },[...e]);
  return {data:n,loading:i,error:s} }
function Zr(t,e=[]){ // subscribe a Query
  ...useEffect(()=>{ if(!t)return; return iN(t,l=>{r(l.docs.map(c=>({id:c.id,...c.data()})))...},...) },[...e]);
  return {data:n,loading:i,error:s} }
```
One-shot `getDoc`/`getDocs` (`ku`/`eu`) are used only inside imperative code paths — mostly inside `runTransaction` callbacks and admin bulk actions.

### Collections found (top-level `collection(db,"<name>")` literals)

`projects`, `entries`, `collections`, `transcriptions`, `translations`, `translationProjects`, `translationTimeline`, `forumPosts`, `forumComments`, `likes`, `conversations` (var `ZO`), `directMessages` (var `eae`), `notifications`, `users`, `emailSettings`, `projectJoins`, `projectJoinRequests`, `projectAnnouncements`, `adminAnnouncements`, `dailyProgress`, `imageCollectionEntries`, `timelineEvents`.
Subcollection found: `translations/{id}/history`.

### `transcriptions` (doc id = `${entryId}_${index}`)

Fields confirmed by client code (superset of the fields already known from the public Firestore query): `entryId, index, canvasId, status, prevStatus, text, tempText, tempTextChanged, notes, tempNotes, annotations, translations, editedBy, syncMode, share, requestReview, approvedBy[], ocr.{ndl,minna}, createdAt, updatedAt`.

Status values observed: `"default"`(implicit initial? not directly observed as a literal), `"initiated"`, `"editing"`, `"completed"`, `"frozen"` (used in the per-project count aggregation: `["completed","initiated","editing","frozen"]`).

Editing lock / presence (this **is** the real-time editing lock the task asked about):
- start editing — `Pyn(entryId,index,uid,syncMode)` — a `runTransaction` that (a) checks the project's `blockedUsers` array and throws if the user is blocked, (b) throws `"Transcription is already being edited"` if `status==="editing"`, else sets:
  ```js
  {status:"editing",prevStatus:l.status,tempText:l.text,tempTextChanged:!1,
   tempNotes:l.notes||[],tempEditedBy:n,syncMode:r||!1,updatedAt:zr()}
  ```
- while editing, the client autosaves every 3 seconds via `setInterval(...Oyn(entryId,index,currentText)...,3e3)`, writing only `tempText`/`tempTextChanged` — i.e. **draft text is streamed live to Firestore**, and if `syncMode` is true, other viewers watching the same doc via `onSnapshot` render the live `tempText` in real time (`b.syncMode&&b.status==="editing"?b.tempText||"":b.text||""`).
- other viewers see the lock live: `xe=…&&a.status==="editing"&&a.tempEditedBy===o.uid` (self editing) vs `Ae=…&&a.tempEditedBy!==o.uid` (someone else editing) → UI strings `"あなたが編集中"` / `"他のユーザーが編集中"` / `"編集されていない"`. Attempting to write while someone else holds the lock throws `"他のユーザーが編集中です。しばらく経ってから再度お試しください。"`
- discard (`Ryn`) reverts `status` to `prevStatus`; save (`Lyn`) validates the lock again, writes `status`(default `"initiated"` unless overridden), `text`, `notes`, `editedBy`, `share`, `requestReview`, `approvedBy` (max 2 reviewers, toggled in/out), computes a char-diff of old vs new text (`ide` = jsdiff) to get a `count` of changed characters, and appends one `timelineEvents` doc (see below).
- OCR result write: `xJe(entryId,index,resultText,engine="ndl")` → `updateDoc({[`ocr.${engine}`]:resultText,updatedAt:zr()})` — confirms `ocr.ndl` and `ocr.minna` are written the same way from the client.
- "recently shared" feed query: `Bn(Sn(rt,"transcriptions"),_n("share","==",!0),di("updatedAt","desc"),Su(150))`.
- deleting an entry cascades: all its `transcriptions` docs are deleted (`Myn`).

### `translations` (a *second*, independent editing/lock system — for the crowd-translation/proofreading module, doc id = entryId)

Fields: `lines:[{id,target,...}], lineCount, tempLines, status:"default"|"editing", tempEditedBy, tempEditedByName, editedBy, editedByName, editCount, projectCode, title, updatedAt`. Same lock semantics as transcriptions (`fHn`=start, `pHn`=autosave tempLines, `mHn`=save+diff+history, `gHn`=discard). On save, a `translations/{id}/history` doc is written (`translationId,uid,displayName,lines,changedLineCount,changedCharCount,comment?,createdAt`) and, if any lines actually changed, a global `translationTimeline` doc (`uid,displayName,entryId,projectCode,title,historyId,changedLineCount,changedCharCount,comment?,createdAt`) plus a **points score `changedCharCount*5`** is returned to the caller (points aren't written to Firestore in this function — presumably surfaced only as a toast/UI value, or applied server-side by a trigger — not confirmed).

### `projects` (doc id = project id)

```js
{id, ...(caller-supplied title/description/etc.), createdAt:zr(), members:[], display:!0,
 totalEntryCount:0, completedEntryCount:0, charCount:0, totalImageCount:0, completedImageCount:0,
 projectType: isPrivate?"private":"user",   // "official" exists as a third value, not settable from this create path
 useOwnGuidelines:!0, functions:{enableOCR:!0,enableTranslations:!0}}
```
Later-added fields (all via `updateDoc`/transaction): `admins:[uid]` (add/remove — `Mjt`/`jjt`), `blockedUsers:[uid]` (add/remove — `$jt`/`Fjt`), `ownerId`, `guidelines`(markdown), `useOwnGuidelines`, `photo`, `markdown`, `keywords`, `functions.enableOCR`, `functions.enableTranslations`. Regular users may own at most **2** projects of type `"user"`/`"private"` combined (`Hjt` checked before allowing creation, `e=2` default). Membership: `Djt`/`Njt` push a uid into `members[]` (creating a `projectJoins` audit doc alongside `Djt`); leaving is done through the `withdrawFromProject` **callable** (not a direct client write — see §6).

### `collections` (a curated grouping of `entries` inside a project; doc id = random)

`{id, projectId, title, description, entryCount:0, display:!0}`. CRUD: `Fyn` create, `Byn` update (merge), `zyn` delete (cascades: deletes every entry in it, and every transcription of each of those entries, before deleting the collection doc).

### `entries` (one manifest/page-set; doc id = random, created only via the `importIiifManifest` **callable**, not a direct client write)

Fields observed: `id, projectId, collectionId, index, label, manifestUrl, thumbnail, size, createdAt`. Update (`FA`) / delete (`b9e`, also decrements `collections.entryCount`) / move-between-collections (`Ejt`, adjusts `entryCount` on both source and destination collections) are direct client writes.

### `likes` — generic polymorphic like/reaction system (doc id = `${collectionName}_${targetId}`)

```js
{collectionName, targetId, likers:[uid,...], uid /* = target doc's own author uid */, createdAt, updatedAt}
```
```js
async function SGt(t,e,n){ // t=collectionName, e=targetId, n=liker uid — toggles
  const r=pz(t,e), i=Xt(rt,t,e); // i = the actual target doc, existence-checked
  return qo(rt, async o=>{ const s=await o.get(r), a=await o.get(i);
    if(!a.exists())throw new Error("Target document not found");
    if(s.exists()){ const u=s.data().likers||[];
      return u.includes(n) ? (o.update(r,{likers:u.filter(d=>d!==n),updatedAt:zr()}),!1)
                            : (o.update(r,{likers:[...u,n],updatedAt:zr()}),!0) }
    else return o.set(r,{collectionName:t,targetId:e,likers:[n],uid:a.data().uid,updatedAt:zr(),createdAt:zr()}),!0 })}
```
Used against `"forumPosts"`, `"forumComments"` and `"timelineEvents"` (i.e. you can like a transcription-activity feed entry directly).

### `forumPosts` / `forumComments`

`forumPosts`: `{uid, projectId (or the sentinel string "translation" for the global translation-module forum, or falsy for the site-wide /forum), title, category?("question"|"request"|"bug"), status:"open"|other, commentCount:0, createdAt, updatedAt}`. Visibility rule (`N$t`): non-project / `"translation"` posts are public; a public (`projectType!=="private"`) project's forum is public; a **private** project's forum is restricted to `ownerId`/`admins[]`/`members[]`.
`forumComments`: `{postId, uid, content, images:[], createdAt, system?:true, systemStatus?, systemActorName?}` — a status change on a post (`$$t`) both updates `forumPosts.status` and injects a synthetic comment with `uid:"SYSTEM"`.
Both support image attachments uploaded to Firebase Storage before the doc is written (max 4 images per post) — see §5.

### `conversations` / `directMessages` — 1:1 and group DM system

```js
// conversations
{participantIds:[uid,...], isGroup:bool, name?(group), createdBy, lastMessageAt, unreadCounts:{uid:count}, createdAt, updatedAt}
// directMessages (flat top-level collection, filtered by conversationId)
{conversationId, senderId, content, deleted:bool, createdAt}
```
`J$t` finds-or-creates a 1:1 conversation; `Q$t` creates a group; `Z$t` adds a participant to a group; `e8t` sends a message (`addDoc`); `t8t` soft-deletes a message (`{deleted:true,content:""}`); `n8t` zeroes a participant's `unreadCounts` entry.

### `notifications` (doc id = random; `uid`-scoped)

`{uid, type, data:{…type-specific…}, state:"unchecked"|"unclicked"|(presumably a 3rd "checked"/clicked state on navigation), createdAt}`. Full `type` enum found in the notification-renderer switch: `"transcription"`, `"projectJoinRequest"`, `"projectJoinRequestApproved"`, `"levelup"`, `"announce"`, `"projectJoin"`, `"forumPost"`, `"like"`, `"forumComment"`, `"directMessage"`. `data` shapes per type (all reconstructed from the renderer components):
| type | data shape |
|---|---|
| `transcription` | `{transcriptionId, entryId, index, uid}` |
| `projectJoinRequest` | `{uid, projectId}` |
| `projectJoinRequestApproved` | `{projectId, projectTitle, status}` |
| `levelup` | `{level}` |
| `announce` | `{projectId, title}` |
| `projectJoin` | `{projectId}` |
| `forumPost` | `{projectId, title, uid, id}` |
| `like` | `{collectionName, targetId}` + top-level `likerUid` |
| `forumComment` | `{postId}` + top-level `uid` (commenter) |
| `directMessage` | `{conversationId, senderId, isGroup?, conversationName?}` |

### `users` (doc id = uid)

See §7 for the full interface. Ranking query: `Bn(Sn(rt,"users"),di(sortField,"desc"),Su(100))` where `sortField` defaults to `"exp"` (toggleable in the ranking UI).

### `projectJoins` / `projectJoinRequests`

`projectJoins`: `{projectId, uid, ownerId, createdAt}` (an audit record written whenever `Djt` adds a member directly). `projectJoinRequests`: `{projectId, uid, message, status:"default"|"denied"|(approved path calls the "withdraw"-sibling flow, not directly observed setting a literal "approved" string, but `IHt`/`DHt` query for `status=="default"` as "pending")}`.

### `projectAnnouncements` / `adminAnnouncements`

`projectAnnouncements`: `{projectId, title, description, createdAt, display:bool, createdBy}`. `adminAnnouncements` (site-wide): `{title, description, createdAt, display:bool}`.

### `dailyProgress`

Top-level, not per-project: `{timestamp, ...}` — queried `orderBy("timestamp","desc")` for the admin "daily trend" chart; exact numeric fields not confirmed (chart-consumer code truncated in the excerpt read).

### `imageCollectionEntries` — personal "clip a region of a page image" bookmarking feature

```js
{entryId, index, uid, uri /* IIIF Image-API region URL */, comment, reading, tags:[string],
 transcriptionId, isPrivate:bool, projectId, xywh:[x,y,w,h] /* pixel region in the source image */, createdAt}
```

### `timelineEvents` — the global/per-project transcription activity feed

Written only from the transcription-save transaction (`Lyn`):
```js
{uid, projectId, entryId, transcriptionId, index, eventType:"transcription",
 data:{...savedTranscriptionFields}, members:project.members, projectType:project.projectType,
 count /* changed-char count */, isReview:bool /* true iff this save fulfilled someone else's requestReview */,
 ...extraOptions, createdAt}
```
(`eventType` has no other observed value in this app's domain.)

---

## 5. Storage

Firebase Storage (`getStorage()`, aliased `V7`) is used for exactly two upload paths found in the bundle, both through a shared `<ImageUploader>` component (`ize`) that does:
```js
const y=async E=>{const C=V7(),k=E.name.split(".").pop(),
  A=`${storagePath}/${Date.now()}_${Math.random().toString(36).substring(2)}.${k}`,
  O=Mse(C,A); return await Nse(O,E), Lse(O)}
```
- **`project_images/{projectId}/{timestamp}_{random}.{ext}`** — a project's cover photo, uploaded from `/adminProject/:id/info`. Accepted types `image/jpeg|png|gif|webp`, default max 5MB (prop-configurable). A temp variant `project_images/temp_{timestamp}/...` is used during the "create new project" flow before the project doc/id exists.
- **`forum_images/{postId-or-projectId}/{timestamp}_{random}.{ext}`** — up to 4 images attached to a forum post, uploaded lazily on submit (not on file selection):
  ```js
  const $=async()=>{ if(y.length===0)return p; const W=V7();
    const G=y.map(async Z=>{ const H=Z.name.split(".").pop(),
      L=`forum_images/${t}/${Date.now()}_${Math.random().toString(36).substring(2)}.${H}`,
      U=Mse(W,L); return await Nse(U,Z), Lse(U) }); return[...p,...await Promise.all(G)]}
  ```
  Forum *comments* have an identical `type="file" accept="image/*" multiple` input (`components.forum.commentList.images`); presumed to use the same `forum_images/...` pattern but the exact call site for comment-image upload was not individually re-verified (**inferred**).
- **No avatar upload** exists — `photoURL` on the `users` doc is set only once, from the OAuth provider's photo, at signup, and is never re-uploaded through Storage.
- The `storageBucket` from the Firebase config is `honkoku3-c466c.firebasestorage.app` (already known).

---

## 6. Cloud Functions / other services

**Callable functions** (`httpsCallable(functions,"name")`, i.e. called through the Firebase SDK's callable protocol at `https://us-central1-honkoku3-c466c.cloudfunctions.net/<name>` under the hood, POST with a Firebase-signed body — not plain REST):
| name | call site / args |
|---|---|
| `importIiifManifest` | `k2(A2,"importIiifManifest")({projectId,collectionId,manifestUrl,label})` — bulk/single entry import from an IIIF Presentation manifest URL |
| `migrateUserData` | referenced by the "v2→v3 user data migration" dialog (`components.dialogs.userDataMigration`); exact call-site args not captured, but the dialog links to `https://v2.honkoku.org/app/#/` and walks the user through migrating their old (V2) account stats into V3 |
| `updateEmailSettings` | `k2(A2,"updateEmailSettings")({email,notifications})` — writes `emailSettings/{uid}` server-side (`{systemAnnouncements,transcriptionOverwrite,forumReply,projectAnnouncement,directMessage}` boolean toggles) |
| `withdrawFromProject` | `k2(A2,"withdrawFromProject")({projectId})` — leaving a project (removes the caller from `members`/possibly `admins` server-side rather than a client array-remove) |
| `adminGetUserAuthMetadata` | site-admin only; fetches Firebase Auth metadata (email, verified, last-sign-in, etc. — not otherwise exposed by the Auth client SDK) for the admin user-search page |

No other `httpsCallable` names were found; the site does **not** use `getFunctions`/`httpsCallable` for anything else (search, entries CRUD, transcriptions, translations are all either direct Firestore writes or the hand-rolled REST/Cloud-Run calls in §3).

**Other hosts contacted:**
- `mp.ex.nii.ac.jp` — National Institute of Informatics "Metom" OCR (§3).
- `ndlkotenocr-lite-web.netlify.app` — linked to, not called programmatically.
- `huggingface.co` — direct browser download of the ONNX model weights.
- `cdn.jsdelivr.net` — `onnxruntime-web` runtime/WASM assets.
- `codh.rois.ac.jp` — linked per-character reference (Center for Open Data in the Humanities kuzushiji character-shape DB), not fetched as an API, only used to build a hyperlink.
- `wiki.honkoku.org` — DokuWiki help/how-to pages, linked only.
- `kula-kuzushiji.web.app` — external kuzushiji-learning site ("navigation.learn"), linked only.
- `v2.honkoku.org` — the legacy (V2) app, linked from the data-migration dialog.
- `x.com/CloudHonkoku` — official X/Twitter account link.
- IIIF manifest/image servers are whatever host each project's `entries.manifestUrl` points to (external museums/libraries) — not a honkoku-controlled host.
- No analytics SDK calls (`getAnalytics`/`logEvent`) were found; instead a plain `gtag.js` snippet is inlined with measurement id `G-9KS10198KN`, and a handful of manual `window.gtag("event", "logout")`-style calls exist.

---

## 7. Data model (reconstructed interfaces)

```ts
type Timestamp = { toDate(): Date } // Firestore Timestamp, or the sentinel returned by serverTimestamp()

interface Project {
  id: string;
  title: string;
  description: string;
  keywords?: string;           // free text, admin-editable
  markdown?: string;           // rich body, MDXEditor-authored
  photo?: string;               // download URL, Storage path `project_images/{id}/...`
  ownerId: string;
  admins?: string[];            // uids, project-level admins (distinct from members)
  members: string[];            // uids
  blockedUsers?: string[];      // uids banned from transcribing in this project
  projectType: "official" | "user" | "private";
  display: boolean;
  useOwnGuidelines: boolean;
  guidelines?: string;          // markdown
  functions: { enableOCR: boolean; enableTranslations: boolean };
  totalEntryCount: number;
  completedEntryCount: number;
  totalImageCount: number;
  completedImageCount: number;
  charCount: number;
  createdAt: Timestamp;
}

interface Collection {           // a curated grouping of Entry inside a Project
  id: string;
  projectId: string;
  title: string;
  description: string;
  entryCount: number;
  display: boolean;
}

interface Entry {                // one IIIF manifest / page-set, created only via importIiifManifest
  id: string;
  projectId: string;
  collectionId: string;
  index: number;                 // order within the collection
  label: string | Record<string, string[]>; // IIIF manifest label, possibly a language map
  manifestUrl: string;
  thumbnail?: string;
  size?: number;                 // page/canvas count
  createdAt: Timestamp;
}

interface Transcription {        // doc id = `${entryId}_${index}`
  id: string;
  entryId: string;
  index: number;
  canvasId?: string;             // IIIF canvas id for this page (per user's own prior finding)
  status: "initiated" | "editing" | "completed" | "frozen" | "default";
  prevStatus?: string;           // restored to `status` on discard
  text: string;
  tempText?: string;             // live draft while status === "editing"
  tempTextChanged?: boolean;
  notes: Note[];
  tempNotes?: (Note | null)[];
  annotations?: unknown[];       // referenced by the API surface but shape not confirmed in this pass
  translations?: unknown;        // per-user prior finding; shape not confirmed in this pass
  ocr?: { ndl?: string; minna?: string };
  editedBy?: string;             // uid
  approvedBy?: string[];         // max 2, review-approval uids
  share?: boolean;
  requestReview?: boolean;
  syncMode?: boolean;            // if true, others watching live see tempText, not just a lock indicator
  tempEditedBy?: string | null;  // uid holding the edit lock
  createdAt?: Timestamp;
  updatedAt: Timestamp;
}

interface Note {                 // element of Transcription.notes / tempNotes
  // exact shape not fully reconstructed; array is nullable-element (deletion = null-out by index)
  [k: string]: unknown;
}

interface Translation {          // "proofread" module; doc id = entryId
  id: string;                    // entryId
  projectCode: string;           // e.g. "KUL"
  title?: string;
  lines: { id: string; target: string }[]; // per-line translation; source text shape not confirmed
  lineCount: number;
  tempLines?: unknown[];
  status: "default" | "editing";
  tempEditedBy?: string | null;
  tempEditedByName?: string | null;
  editedBy?: string;
  editedByName?: string;
  editCount: number;
  updatedAt: Timestamp;
}

interface TranslationHistory {   // subcollection translations/{id}/history
  translationId: string;
  uid: string;
  displayName: string;
  lines: Translation["lines"];
  changedLineCount: number;
  changedCharCount: number;
  comment?: string;
  createdAt: Timestamp;
}

interface User {                 // doc id = uid
  uid: string;
  displayName: string;
  profile: string;               // bio text
  website: string;                // present in schema, no edit UI found
  photoURL: string;               // from OAuth provider only, never re-uploaded
  level: number;                  // gamification level, starts at 1
  exp: number;                    // experience points; ranking default sort field
  likeCount: number;
  charCount: number;              // transcribed character count
  stoneCount: number;             // "石" — a second gamification currency
  createdAt: Timestamp;
}

interface EmailSettings {        // doc id = uid, written only via the updateEmailSettings callable
  email: string;
  notifications: {
    systemAnnouncements: boolean;
    transcriptionOverwrite: boolean;
    forumReply: boolean;
    projectAnnouncement: boolean;
    directMessage: boolean;
  };
}

interface Like {                 // polymorphic; doc id = `${collectionName}_${targetId}`
  collectionName: string;        // "forumPosts" | "forumComments" | "timelineEvents" | ...
  targetId: string;
  likers: string[];              // uids
  uid: string;                   // author uid of the liked document
  createdAt: Timestamp;
  updatedAt: Timestamp;
}

interface ForumPost {
  id: string;
  uid: string;
  projectId?: string | "translation"; // absent/undefined = site-wide /forum
  title: string;
  content?: string;
  images?: string[];             // Storage download URLs, forum_images/{id}/...
  category?: "question" | "request" | "bug";
  status: "open" | string;       // other values set only via system status-change flow
  commentCount: number;
  createdAt: Timestamp;
  updatedAt: Timestamp;
}

interface ForumComment {
  id: string;
  postId: string;
  uid: string;                   // "SYSTEM" for auto-generated status-change comments
  content: string;
  images: string[];
  system?: boolean;
  systemStatus?: string;
  systemActorName?: string;
  createdAt: Timestamp;
}

interface Conversation {
  id: string;
  participantIds: string[];
  isGroup: boolean;
  name?: string;                 // group name
  createdBy: string;
  lastMessageAt: Timestamp;
  unreadCounts: Record<string, number>; // per-uid unread count
  createdAt: Timestamp;
  updatedAt: Timestamp;
}

interface DirectMessage {
  id: string;
  conversationId: string;
  senderId: string;
  content: string;
  deleted: boolean;              // soft-delete; content is blanked, not the doc
  createdAt: Timestamp;
}

interface Notification {
  id: string;
  uid: string;                   // recipient
  type: "transcription" | "projectJoinRequest" | "projectJoinRequestApproved" | "levelup"
      | "announce" | "projectJoin" | "forumPost" | "like" | "forumComment" | "directMessage";
  data: Record<string, unknown>; // shape depends on `type`, see §4 table
  state: "unchecked" | "unclicked" | string; // "unchecked" -> (opened dropdown) "unclicked" -> (clicked) presumably "checked"
  createdAt: Timestamp;
}

interface ImageCollectionEntry {  // personal bookmark of a cropped region of a page image
  id: string;
  entryId: string;
  index: number;
  uid: string;
  uri: string;                    // IIIF Image API region URL
  comment: string;
  reading: string;                 // furigana/reading the user recorded for the clipped text
  tags: string[];
  transcriptionId: string;
  isPrivate: boolean;
  projectId: string;
  xywh: [number, number, number, number]; // pixel crop region in the source image
  createdAt: Timestamp;
}

interface TimelineEvent {
  id: string;
  uid: string;
  projectId: string;
  entryId: string;
  transcriptionId: string;
  index: number;
  eventType: "transcription";
  data: Transcription;            // snapshot of the saved transcription
  members: string[];              // project.members at save time (denormalized)
  projectType: Project["projectType"];
  count: number;                  // changed-character count for this save
  isReview: boolean;
  share?: boolean;
  requestReview?: boolean;
  isApproval?: boolean;
  createdAt: Timestamp;
}

interface ProjectJoinRequest {
  id: string;
  projectId: string;
  uid: string;
  message: string;
  status: "default" | "denied" | string; // "default" == pending
  createdAt: Timestamp;
}

interface ProjectJoin {           // audit trail written when a member is added directly
  projectId: string;
  uid: string;
  ownerId: string;
  createdAt: Timestamp;
}

interface Announcement {          // shape shared by projectAnnouncements and adminAnnouncements
  id: string;
  projectId?: string;             // absent on adminAnnouncements (site-wide)
  title: string;
  description: string;
  display: boolean;
  createdBy?: string;
  createdAt: Timestamp;
}
```

---

## 8. Editor/markup behaviour

The transcription body is a **plain-text CodeMirror 6 buffer**; the app's special notation is a bespoke markup language layered on top of plain Unicode/full-width punctuation, parsed by what looks like an **ANTLR4-generated recursive-descent parser** (`pRe.CommonToken`, `tokenSource.nextToken()`, `readClass()`, `readKaeriten()`, `readOkurigana()`, `isBracket()`) into an AST used for:
- live syntax-highlighted/preview rendering (an `htmlTemplate` per element type),
- plain-text re-serialization (`textTemplate`),
- LaTeX export (`latexTemplate`),
- Word/OOXML export (literal `<w:r><w:t>{{$text}}</w:t></w:r>` templates),
- and XML export for at least some elements (`xmlTemplate`, e.g. TEI-`<gap>`-shaped for 虫損).

Full element catalog extracted from the grammar's element registry (`elemName`/`type`/`doc`/`example`):

| elemName | type | notation | meaning |
|---|---|---|---|
| 人物 | inline | `｛大石内蔵助｝` | person name |
| 場所 | inline | `〔日本橋〕` | place name |
| 日時 | inline | `＜安政二年卯十月二日＞` | date/time |
| 題 | inline | `《題：地震年代記》` | document title |
| 割書 | inline | `《割書：一行目｜二行目｜三行目》` (2–4 segments, `｜`-separated) | split/interlinear writing (warigaki) |
| 振り仮名 | inline | `未（いまだ｜ズ）` (2–3 segments) | furigana / kanbun re-reading marks, same syntax for both |
| 圏点 | inline | `《圏点：本文｜﹅》` | emphasis dots, 2nd segment is the dot glyph used |
| 返り点 | inline | `｛＿レ｝` (also `＿一＿二＿三＿上＿中＿下`) | kanbun kaeriten reading-order marks |
| 送り仮名 | inline | `￣オクリガナ` | kanbun okurigana |
| 見せ消ち | inline | `《見せ消ち：訂正された箇所｜追記された箇所》` | struck-through-but-legible correction |
| 注釈 | inline | `【このようにして注釈を書きます】` | transcriber's own annotation/question |
| 脚注 | inline | `＃１０` | footnote marker |
| 虫損 | inline | `虫損した□文字` (□ count = illegible char count) | insect/worm damage gap |
| 難読 | inline | `読めない■■文字` | illegible/hard-to-read characters |
| 字下げ一／二／三 | block | `％字下げ一 … ` | 1/2/3-character indent block |
| 表紙 | block | `％表紙 … ` | cover-page block |

Additional raw tokens seen in the lexer's symbol table (used inside the above constructs or standalone): `｛ ｝ 〔 〕 ＜ ＞ ［ ＿ ￣ ■ □`, plus internal pseudo-tags `<TATE>` (rendered as `ー`, i.e. a vertical-writing long-vowel mark) and `<BLOCK>` (stripped on normalization) — evidence of **tategaki (vertical writing) awareness** in the text-normalization step:
```js
e=e.replace(/<TATE>/g,"ー"),e=e.replace(/<BLOCK>/g,"")
```
No direct evidence of a CSS `writing-mode: vertical-rl` rendering path was found in this pass (not ruled out — `app-bundle.css` was not searched); only this text-normalization hint was confirmed.

**Character recognition assist** (separate from the notation grammar): clicking/selecting a character can crop it and send it to the Metom API (§3/§6) or, for a whole page, run the in-browser ONNX kuzushiji model — both return `{character, score, rank}` candidates the user can pick to insert.

**Page image display**: OpenSeadragon deep-zoom viewer over IIIF Image API tiles (2.x/3.x compliance auto-negotiated), sourced from each `Entry.manifestUrl`'s IIIF Presentation manifest. A "download full-size image" mode exists (routes through the `imageproxy` Cloud Run service to work around CORS). An `xywh`-region cropping tool feeds the "image collection" bookmarking feature (§4).

---

## 9. Search

No Algolia/Typesense/Meilisearch. Full-text transcription search and user search are both served by the app's own **`search` Cloud Run/Functions service** (§3) — presumably backed by Firestore queries or a custom index maintained by that service (its internals are obviously not visible from the client bundle). The personal "image collection" bookmarks have their own search endpoint on the sibling `search` Cloud Function (`/image-collection-entries`). No client-side Algolia/Firestore-composite-index full-text search code was found.

---

## 10. Gamification / social

- **Per-user stats** (`users` doc): `level`, `exp` (drives level-ups — a `"levelup"` notification fires with `{level}` and shows "レベルアップしました！Lv.{n}に到達しました！"), `likeCount`, `charCount`, `stoneCount` ("石"/"stones", a second currency — accrual mechanism not located in this pass).
- **Leaderboard/ranking**: `Bn(Sn(rt,"users"),orderBy(sortField,"desc"),limit(100))`, default `sortField="exp"`, user-togglable in the ranking widget (labels seen: `charUnit`, `likeUnit`("likes"), `pointUnit`("pt")).
- **Translation-module points**: `changedCharCount * 5` computed client-side per save (not confirmed to be persisted as a `points` field anywhere; likely surfaced only in a toast, or applied by a server-side trigger not visible here).
- **Activity feed**: `timelineEvents` (global + per-project via `projectId`/`members` filtering) — every transcription save is one event; `translationTimeline` is the equivalent feed for the proofreading module.
- **Likes**: generic polymorphic `likes` collection (§4), usable on `forumPosts`, `forumComments`, and `timelineEvents`.
- **Forum**: `forumPosts`/`forumComments`, with categories (`question`/`request`/`bug`), post status workflow with system-generated status-change comments, image attachments (max 4/post), and private-project-scoped visibility.
- **Direct messages**: 1:1 and group conversations with unread counters and soft-delete (§4).
- **Notifications**: 10 types (§4/§7), unread badge counted via `getCountFromServer` on `state=="unchecked"`.
- **No "follow"/"badge" system was found** — no `follows`/`badges`/`achievements` collection or UI copy of that shape turned up in ~1770 i18n keys searched.

---

## 11. Open questions (need a logged-in network capture to confirm)

1. **`/api/projects`, `/api/projects/{id}`, `/api/collections/{id}`, `/api/entries/{id}`** — not called anywhere in this bundle. Need to confirm from a live network trace whether these are used by: server-side rendering/OG-tag generation, a separate mobile/partner client, or are simply a public read API offered independently of the SPA. Their request/response shape is entirely unconfirmed from this artifact.
2. Whether `search-r7au5bknyq-uc.a.run.app` and `us-central1-honkoku3-c466c.cloudfunctions.net/search` are truly the same backend (stated above as a strong inference from GCP 2nd-gen Cloud Functions deploy behavior, not proven).
3. Exact request/response body of `migrateUserData`, `adminGetUserAuthMetadata`, and the precise mutation performed server-side by `withdrawFromProject` (e.g., does it also touch `admins[]`, does it write an audit doc).
4. Whether `ocr.ndl` results are ever populated by an automated pipeline (batch job hitting the real NDL OCR-Lite model) versus purely manual copy-paste by transcribers from the linked external tool — the client only ever *reads/writes* `transcriptions.ocr.ndl`, it never calls an NDL OCR endpoint itself.
5. Exact shape of `Transcription.notes[i]`, `Transcription.annotations`, and `Transcription.translations` (all referenced in the public Firestore schema per the task brief, but this pass did not locate the client code that constructs a single `Note`/`annotation` object literal with all its fields — only array-level operations were found).
6. `Translation.lines[i]` — confirmed `{id, target}`; whether it also carries a `source` (original transcribed text) field per line, or the source is looked up separately from the linked `Transcription`, was not confirmed.
7. Precise numeric field(s) inside `dailyProgress` documents (only the collection name and sort field `timestamp` were confirmed; the chart-consuming component's field access wasn't fully traced).
8. Whether Firebase App Check is actually enabled (the Auth/Firestore SDK ships generic App-Check-token plumbing regardless of whether the app calls `initializeAppCheck`; no explicit `ReCaptchaV3Provider`/site-key literal was found, so this may be dead SDK code rather than an active protection — a live network trace would show an `X-Firebase-AppCheck` header, or its absence, on Firestore/Functions calls).
9. Whether project `"official"` status is ever settable from any UI reached by a normal user, or is exclusively a backend/console operation (only `"user"`/`"private"` were seen written from client code).
10. Exact accrual rule for `stoneCount` ("石") — no code path incrementing it was found in this pass (only its initial `0` at signup and its read-only display).
11. `app-bundle.css` was not examined in this pass — any `writing-mode`/vertical-text CSS, and MUI/theme details (colors, dark mode), remain unconfirmed beyond what is inferable from `sx` prop literals in the JS.
