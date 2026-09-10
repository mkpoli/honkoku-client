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
  const limit = Number(args.limit ?? 20);
  if (
    [
      "page_lock",
      "page_draft",
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
    return fixtureEdit(
      command,
      args,
      required(p),
      signedIn ? whoami : null,
      e,
      editingEvents,
    );
  }
  switch (command) {
    case "list_projects":
      return structuredClone(projects);
    case "get_project":
      return required(
        args.id === project.id
          ? project
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
      return [...ranking]
        .sort((a, b) => (b[key] ?? 0) - (a[key] ?? 0))
        .slice(0, limit);
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
