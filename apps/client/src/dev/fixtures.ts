import notesFixture from "../../../../fixtures/api/page-notes.json";
import pageHistoryFixture from "../../../../fixtures/home/page-history.json";
import bibliographyFixture from "../../../../fixtures/api/manifest-bibliography-v3.json";
import glyphFixture from "../../../../fixtures/glyphs/attestations-候.json";
import clipFixture from "../../../../fixtures/glyphs/clips.json";
import type { Clip, ClipInput } from "@honkoku/client-api/types";
import pageStatuses from "../../../../fixtures/home/page-statuses.json";
import historyFixture from "../../../../fixtures/home/history.json";
import type { RecentWork } from "@honkoku/client-api/types";
import localOcr from "../../../../fixtures/api/ocr-local-0916dafb-3.json";
import collectionFigures from "../../../../fixtures/home/collection-progress-ainu.json";
import entrySummaries from "../../../../fixtures/api/entry-summaries-ainu.json";
import selectedSummaries from "../../../../fixtures/api/entries-3R4VhlBfvOYeqPY13cJm.json";
import entryFigures from "../../../../fixtures/home/entry-progress-ainu.json";
import alignmentFixture from "../../../../fixtures/api/page-minna-ocr.json";
import alignmentEntry from "../../../../fixtures/api/entry-minna-ocr.json";
import unreadCount from "../../../../fixtures/home/unread-notification-count.json";
import { fixtureEdit } from "./editing";
import pagesJson from "../../../../fixtures/home/pages.json";
import { normalizeCanvas } from "../../../../packages/client-api/canvas";
import projectsJson from "../../../../fixtures/api/projects.json";
import projectJson from "../../../../fixtures/api/project-ainu.json";
import collectionJson from "../../../../fixtures/api/collection-3R4VhlBfvOYeqPY13cJm.json";
import entryJson from "../../../../fixtures/api/entry-0916dafb80cdc48ca7687afcad4a4f35.json";
import firestorePages from "../../../../fixtures/api/firestore-pages-0916dafb.json";
import timelineJson from "../../../../fixtures/home/timeline.json";
import projectTimelineJson from "../../../../fixtures/home/timeline-ainu.json";
import rankingSelf from "../../../../fixtures/home/ranking-self.json";
import rankingJson from "../../../../fixtures/home/ranking.json";
import announcements from "../../../../fixtures/home/announcements.json";
import whoami from "../../../../fixtures/home/whoami.json";
import catalogJson from "../../../../fixtures/home/catalog.json";
import daily from "../../../../fixtures/home/daily-progress.json";
import type {
  Collection,
  Entry,
  Page,
  Project,
  TimelineFilter,
  TimelineItem,
  User,
} from "@honkoku/client-api/types";

function normalize(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(normalize);
  if (value && typeof value === "object") {
    const v = value as Record<string, unknown>;
    if (typeof v._seconds === "number")
      return new Date(
        v._seconds * 1000 + Number(v._nanoseconds ?? 0) / 1e6,
      ).toISOString();
    return Object.fromEntries(
      Object.entries(v).map(([k, v]) => [k, normalize(v)]),
    );
  }
  return value;
}
function decode(value: unknown): unknown {
  const v = value as Record<string, unknown>;
  if ("arrayValue" in v)
    return ((v.arrayValue as { values?: unknown[] }).values ?? []).map(decode);
  if ("mapValue" in v)
    return fields(
      (v.mapValue as { fields?: Record<string, unknown> }).fields ?? {},
    );
  if ("integerValue" in v) return Number(v.integerValue);
  return Object.values(v)[0];
}
function fields(value: Record<string, unknown>): Record<string, unknown> {
  return Object.fromEntries(
    Object.entries(value).map(([k, v]) => [k, decode(v)]),
  );
}
const projects = normalize(projectsJson) as Project[];
const project = normalize(projectJson) as Project;
const catalog = normalize(catalogJson) as {
  collections: Collection[];
  entries: Entry[];
  users: User[];
};
const collections = [
  normalize(collectionJson) as Collection,
  ...catalog.collections.filter((c) => c.id !== collectionJson.id),
];
const entry = normalize(entryJson) as Entry;
const entries = [
  entry,
  normalize(alignmentEntry) as Entry,
  ...catalog.entries.filter((e) => e.id !== entry.id),
].map((e) => ({ ...e, canvases: e.canvases?.map(normalizeCanvas) }));
const pages = new Map((entry.transcriptions ?? []).map((p) => [p.index, p]));
for (const row of firestorePages)
  if ("document" in row && row.document) {
    const p = fields(row.document.fields) as unknown as Page;
    pages.set(p.index, p);
  }
for (const override of pageStatuses) {
  const page = pages.get(override.index);
  if (page) Object.assign(page, override);
}
if (sessionStorage.getItem("honkoku.fixture.notes") === "true")
  pages.set(notesFixture.index, normalize(notesFixture) as Page);
const extraPages = normalize(pagesJson) as Record<string, Page[]>;
extraPages[alignmentEntry.id] = alignmentEntry.canvases.map((canvas, index) =>
  index === alignmentFixture.page.index
    ? (normalize(alignmentFixture.page) as Page)
    : {
        id: `${alignmentEntry.id}_${index}`,
        entryId: alignmentEntry.id,
        index,
        canvasId: canvas.id,
        status: "default",
        text: "",
        notes: [],
      },
);
const timeline = normalize(timelineJson) as TimelineItem[];
const projectTimeline = normalize(projectTimelineJson) as TimelineItem[];
const editingEvents: TimelineItem[] = [];
const ranking = normalize(rankingJson) as User[];
const users = new Map(
  [...catalog.users, ...ranking, whoami].map((u) => [u.uid, u]),
);
let recentHistory: RecentWork[] = JSON.parse(
  sessionStorage.getItem("honkoku.fixture.history") ??
    JSON.stringify(historyFixture),
);
function recordHistory(
  e: Entry,
  p: Pick<Page, "index" | "status">,
  saved: boolean,
) {
  const now = new Date().toISOString();
  const previous = recentHistory.find(
    (row) => row.entryId === e.id && row.index === p.index,
  );
  recentHistory = [
    {
      entryId: e.id,
      index: p.index,
      projectId: e.projectId,
      openedAt: saved ? (previous?.openedAt ?? now) : now,
      savedAt: saved ? now : (previous?.savedAt ?? null),
      statusAfter: p.status,
      entryLabel: e.label,
      projectTitle: projects.find((p) => p.id === e.projectId)?.title ?? null,
      thumbnail: e.thumbnail ?? null,
      nextUnfinishedIndex: null,
    },
    ...recentHistory.filter((row) => row.entryId !== e.id),
  ];
  sessionStorage.setItem(
    "honkoku.fixture.history",
    JSON.stringify(recentHistory),
  );
}
let fixtureClips = structuredClone(clipFixture) as Clip[];
let signedIn = sessionStorage.getItem("honkoku.fixture.signedOut") !== "true";
function required<T>(value: T | undefined): T {
  if (!value)
    throw {
      kind: "fixture",
      message:
        "ブラウザープレビューにはサンプルデータのみ収録しています。デスクトップアプリではすべての資料を閲覧できます。",
    };
  return value;
}
export async function fixtureInvoke(
  command: string,
  args: Record<string, unknown>,
): Promise<unknown> {
  if (!import.meta.env.DEV) throw new Error("閲覧データを利用できません。");
  const delay = window.honkokuFixtureDelays?.[command];
  if (delay) await new Promise((resolve) => setTimeout(resolve, delay));
  const limit = Number(args.limit ?? 20);
  if (
    [
      "page_lock",
      "page_draft",
      "page_draft_notes",
      "page_note_delete",
      "page_save",
      "page_discard",
      "page_lock_state",
    ].includes(command)
  ) {
    const e = required(entries.find((e) => e.id === args.entryId));
    const index = Number(args.index);
    let p =
      args.entryId === entry.id
        ? pages.get(index)
        : extraPages[e.id]?.find((p) => p.index === index);
    if (!p && index >= 0 && index < (e.size ?? 0)) {
      p = {
        id: `${e.id}_${index}`,
        entryId: e.id,
        index,
        status: "default",
        text: "",
        notes: [],
      };
      if (e.id === entry.id) pages.set(index, p);
      else (extraPages[e.id] ??= []).push(p);
    }
    const result = fixtureEdit(
      command,
      args,
      required(p),
      signedIn ? whoami : null,
      e,
      editingEvents,
    );
    if (
      ["page_draft", "page_draft_notes", "page_note_delete"].includes(command)
    ) {
      const writes = JSON.parse(
        sessionStorage.getItem("honkoku.fixture.noteWrites") ?? "[]",
      );
      writes.push({ command, tempNotes: structuredClone(p?.tempNotes) });
      sessionStorage.setItem(
        "honkoku.fixture.noteWrites",
        JSON.stringify(writes),
      );
    }
    if (command === "page_save") recordHistory(e, required(p), true);
    return result;
  }
  switch (command) {
    case "page_history":
      return args.entryId === entry.id && Number(args.index) === 3
        ? structuredClone(pageHistoryFixture).slice(0, limit)
        : [];
    case "entry_bibliography":
      return structuredClone(bibliographyFixture);
    case "save_transcription":
    case "save_full_image":
      throw {
        kind: "fixture",
        message: "ファイルの保存はデスクトップアプリで利用できます。",
      };
    case "glyph_attestations": {
      const result = structuredClone(glyphFixture);
      result.pages = result.pages
        .filter((p) => !args.project || p.projectId === args.project)
        .slice(0, Number(args.limit));
      if (args.character !== "候") result.pages = [];
      result.total = result.pages.length;
      return result;
    }
    case "clips_list":
      if (!signedIn)
        throw { kind: "signed_out", message: "ログインしてください。" };
      return fixtureClips;
    case "clip_create": {
      if (!signedIn)
        throw { kind: "signed_out", message: "ログインしてください。" };
      const input = args.input as ClipInput;
      const canvas = entries.find((e) => e.id === input.entryId)?.canvases?.[
        input.index
      ];
      if (!canvas?.infoJsonUrl) throw Error("IIIF画像がありません。");
      const clip: Clip = {
        ...input,
        id: crypto.randomUUID(),
        uid: whoami.uid,
        createdAt: new Date().toISOString(),
        projectId: entry.projectId,
        transcriptionId: `${input.entryId}_${input.index}`,
        uri: `${canvas.infoJsonUrl.slice(0, -10)}/${input.xywh.join(",")}/full/0/default.jpg`,
      };
      fixtureClips = [clip, ...fixtureClips];
      return clip;
    }
    case "clip_delete":
      if (!signedIn)
        throw { kind: "signed_out", message: "ログインしてください。" };
      fixtureClips = fixtureClips.filter((c) => c.id !== args.id);
      return;

    case "history_recent":
      return recentHistory.slice(0, Math.max(0, limit)).map((row) => {
        const e = entries.find((e) => e.id === row.entryId);
        const ps =
          row.entryId === entry.id
            ? [...pages.values()]
            : (extraPages[row.entryId] ?? []);
        const next = Array.from(
          { length: e?.size ?? 0 },
          (_, index) => index,
        ).find((index) => {
          const p = ps.find((p) => p.index === index);
          return !p || p.status === "default" || p.status === "initiated";
        });
        return { ...row, nextUnfinishedIndex: next ?? null };
      });
    case "history_clear":
      recentHistory = [];
      sessionStorage.setItem("honkoku.fixture.history", "[]");
      return;
    case "history_open": {
      const e = required(entries.find((e) => e.id === args.entryId));
      const index = Number(args.index);
      const p =
        e.id === entry.id
          ? pages.get(index)
          : extraPages[e.id]?.find((p) => p.index === index);
      recordHistory(e, p ?? { index, status: "default" }, false);
      return;
    }
    case "project_page_activity": {
      const latest: Record<string, string> = {};
      for (const p of [
        ...pages.values(),
        ...Object.values(extraPages).flat(),
      ]) {
        if (
          entries.find((e) => e.id === p.entryId)?.projectId !==
            args.projectId ||
          !p.updatedAt
        )
          continue;
        if (
          !latest[p.entryId] ||
          Date.parse(p.updatedAt) > Date.parse(latest[p.entryId])
        )
          latest[p.entryId] = p.updatedAt;
      }
      return latest;
    }
    case "editing_pages":
      return [...pages.values(), ...Object.values(extraPages).flat()]
        .filter(
          (p) =>
            signedIn && p.status === "editing" && p.tempEditedBy === whoami.uid,
        )
        .map((p) => structuredClone(p));
    case "ocr_diagnostics":
      return {
        status: await fixtureInvoke("ocr_status", {}),
        environment_ready: true,
        models_present: true,
        models_directory_exists: true,
        models_bytes: 303038464,
        last_error: null,
        log_path: "~/.local/share/org.honkoku.client/logs/ocr.log",
      };
    case "ocr_doctor":
      return "honkoku-ocr-py 0.3.0\nonnxruntime: CUDAExecutionProvider\nmodel v18: verified";
    case "ocr_repair_models":
    case "ocr_status":
    case "ocr_setup":
      return {
        version: "0.3.0",
        device: "cuda",
        environment_ready: true,
        models_ready: true,
        model_version: "v18",
        cuda_available: true,
      };
    case "ocr_result":
    case "ocr_run_page":
      return args.entryId === entry.id && Number(args.index) === 3
        ? structuredClone(localOcr)
        : null;
    case "ocr_cancel":
      return;
    case "ocr_publish_page":
      return;

    case "list_projects":
      return structuredClone(projects);
    case "get_project":
      return required(
        args.id === project.id
          ? {
              ...project,
              useOwnGuidelines: false,
              guidelines:
                "## アイヌ語資料の翻刻\n原本の表記を尊重し、判読に迷った箇所は注記に残してください。",
            }
          : projects.find((p) => p.id === args.id),
      );
    case "list_collections": {
      const rows = collections.filter((c) => c.projectId === args.projectId);
      return required(
        rows.length || args.projectId === project.id ? rows : undefined,
      );
    }
    case "get_collection":
      return required(collections.find((c) => c.id === args.id));
    case "list_entry_summaries": {
      required(
        collectionFigures.find((p) => p.collectionId === args.collectionId),
      );
      return args.collectionId === collectionJson.id
        ? selectedSummaries
        : entrySummaries.filter((e) => e.collectionId === args.collectionId);
    }
    case "collection_progress":
      return required(
        collectionFigures.find((p) => p.collectionId === args.collectionId),
      );
    case "entry_progress":
      return (args.entryIds as string[]).map((id) =>
        required(entryFigures.find((p) => p.entryId === id)),
      );
    case "get_entry":
      return required(entries.find((e) => e.id === args.id));
    case "list_pages":
      return args.entryId === entry.id
        ? [...pages.values()].sort((a, b) => a.index - b.index)
        : required(extraPages[String(args.entryId)]);
    case "iiif_prepare_entry":
      return (
        required(entries.find((e) => e.id === args.entryId)).canvases ?? []
      );
    case "recognize_region":
      if (window.honkokuRecognitionError)
        throw { kind: "recognition", message: window.honkokuRecognitionError };
      window.honkokuRecognitionRegion = args.xywh as number[];
      return [..."候侯健倹供使侍何作仁"].map((character, i) => ({
        character,
        probability: i === 0 ? 0.82 : 0.18 / 9,
      }));
    case "glyph_image_url":
    case "iiif_local_url":
      return args.url;
    case "session_import":
      signedIn = true;
      sessionStorage.removeItem("honkoku.fixture.signedOut");
      return {
        uid: whoami.uid,
        display_name: whoami.displayName,
        providers: [],
      };
    case "session_clear":
      signedIn = false;
      sessionStorage.setItem("honkoku.fixture.signedOut", "true");
      return;
    case "session_current":
      return signedIn
        ? { uid: whoami.uid, display_name: whoami.displayName, providers: [] }
        : null;
    case "me":
      return signedIn ? whoami : null;
    case "get_user":
      return required(users.get(String(args.uid)));
    case "unread_notification_count":
      return signedIn ? unreadCount.count : 0;
    case "home_daily_progress":
      return daily;
    case "home_announcements":
      return announcements.slice(0, limit);
    case "home_ranking": {
      const key = (args.sort ?? "exp") as "exp" | "charCount" | "likeCount";
      const outside =
        sessionStorage.getItem("honkoku.fixture.rankingSelf") === "outside";
      return ranking
        .filter((user) => !outside || user.uid !== whoami.uid)
        .sort((a, b) => (b[key] ?? 0) - (a[key] ?? 0))
        .slice(0, limit);
    }
    case "home_ranking_self": {
      if (!signedIn)
        throw { kind: "signed_out", message: "ログインしてください。" };
      const field = args.sort as keyof typeof rankingSelf;
      if (!(field in rankingSelf))
        throw { kind: "invalid", message: "集計項目が正しくありません。" };
      return sessionStorage.getItem("honkoku.fixture.rankingUncounted") ===
        "true"
        ? { rank: null, value: null }
        : rankingSelf[field];
    }
    case "home_timeline": {
      const filter = (args.filter ?? {}) as TimelineFilter;
      const joined = projects
        .filter(
          (p) =>
            p.members?.includes(whoami.uid) || p.admins?.includes(whoami.uid),
        )
        .map((p) => p.id);
      return [
        ...editingEvents,
        ...(filter.project_id === "ainu" ? projectTimeline : timeline),
      ]
        .filter(
          (i) =>
            (!filter.project_id || i.event.projectId === filter.project_id) &&
            (!filter.joined ||
              (signedIn && joined.includes(i.event.projectId))) &&
            (!filter.before ||
              i.event.createdAt < filter.before ||
              (i.event.createdAt === filter.before &&
                !!filter.before_id &&
                i.event.id < filter.before_id)),
        )
        .slice(0, limit);
    }
    default:
      throw { kind: "command", message: "この操作には対応していません。" };
  }
}

declare global {
  interface Window {
    honkokuFixtureDelays?: Record<string, number>;
    honkokuRecognitionError?: string;
    honkokuRecognitionRegion?: number[];
  }
}
