import { invoke as nativeInvoke } from "@tauri-apps/api/core";
import type {
  Project,
  Collection,
  Entry,
  Page,
  SessionInfo,
  TimelineFilter,
  TimelineItem,
  RankingSort,
  RankingSelf,
  User,
  Announcement,
  DailyProgress,
} from "./types";
export const isTauri = () =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
export interface ConnectionOutcome {
  connected: boolean;
  syncedAt?: number;
}
export async function invoke<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  try {
    let result: T;
    if (isTauri()) result = await nativeInvoke<T>(command, args);
    else if (import.meta.env.DEV) {
      const { fixtureInvoke } =
        await import("../../apps/client/src/dev/fixtures");
      result = (await fixtureInvoke(command, args)) as T;
    } else
      throw {
        kind: "transport",
        message: "デスクトップアプリで開いてください。",
      };
    window.dispatchEvent(
      new CustomEvent<ConnectionOutcome>("honkoku:connection", {
        detail: { connected: true, syncedAt: Date.now() },
      }),
    );
    return result;
  } catch (error) {
    const kind =
      typeof error === "object" && error !== null && "kind" in error
        ? String((error as { kind: unknown }).kind)
        : "";
    if (kind === "network" || kind === "timeout" || kind === "transport")
      window.dispatchEvent(
        new CustomEvent<ConnectionOutcome>("honkoku:connection", {
          detail: { connected: false },
        }),
      );
    throw error;
  }
}
export const listProjects = () => invoke<Project[]>("list_projects");
export const getProject = (id: string) =>
  invoke<Project>("get_project", { id });
export const listCollections = (projectId: string) =>
  invoke<Collection[]>("list_collections", { projectId });
export const getCollection = (id: string) =>
  invoke<Collection>("get_collection", { id });
export const getEntry = (id: string) => invoke<Entry>("get_entry", { id });
export const listPages = (entryId: string) =>
  invoke<Page[]>("list_pages", { entryId });
export const sessionImport = () => invoke<SessionInfo>("session_import");
export const sessionCurrent = () =>
  invoke<SessionInfo | null>("session_current");
export const sessionClear = () => invoke<void>("session_clear");
export const homeTimeline = (filter: TimelineFilter = {}, limit = 20) =>
  invoke<TimelineItem[]>("home_timeline", { filter, limit });
export const homeRanking = (sort: RankingSort = "exp", limit = 100) =>
  invoke<User[]>("home_ranking", { sort, limit });
export const homeRankingSelf = (sort: RankingSort = "exp") =>
  invoke<RankingSelf>("home_ranking_self", { sort });
export const homeAnnouncements = (limit = 5) =>
  invoke<Announcement[]>("home_announcements", { limit });
export const homeDailyProgress = () =>
  invoke<DailyProgress[]>("home_daily_progress");
export const me = () => invoke<User | null>("me");
export const unreadNotificationCount = () =>
  invoke<number>("unread_notification_count");
export const getUser = (uid: string) => invoke<User>("get_user", { uid });

export const pageLock = (entryId: string, index: number, syncMode = false) =>
  invoke<Page>("page_lock", { entryId, index, syncMode });
export const pageDraft = (entryId: string, index: number, text: string) =>
  invoke<Page>("page_draft", { entryId, index, text });
export const pageSave = (
  entryId: string,
  index: number,
  options: import("./types").SaveOptions = {},
) => invoke<import("./types").SavedPage>("page_save", { entryId, index, options });
export const pageDiscard = (entryId: string, index: number) =>
  invoke<void>("page_discard", { entryId, index });
export const pageLockState = (entryId: string, index: number) =>
  invoke<import("./types").PageLockState>("page_lock_state", { entryId, index });

export async function onWindowClose(
  handler: (event: { preventDefault: () => void }, close: () => Promise<void>) => void | Promise<void>,
) {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const appWindow = getCurrentWindow();
  return appWindow.onCloseRequested((event) => handler(event, () => appWindow.close()));
}

export const sessionSignIn = (provider: import("./types").SignInProvider) =>
  invoke<void>("session_sign_in", { provider });
export const sessionClearWithSite = (clearSiteData: boolean) =>
  invoke<void>("session_clear", { clearSiteData });
export async function onSessionChanged(handler: () => void) {
  const { listen } = await import("@tauri-apps/api/event");
  return listen("session-changed", handler);
}
export async function onSignInClosed(handler: () => void) {
  const { listen } = await import("@tauri-apps/api/event");
  return listen("signin-closed", handler);
}

export const listEntrySummaries = (collectionId: string) =>
  invoke<import("./types").EntrySummary[]>("list_entry_summaries", { collectionId });
export const collectionProgress = (collectionId: string, refresh = false) =>
  invoke<import("./types").CollectionProgress>("collection_progress", { collectionId, refresh });
export const entryProgress = (entryIds: string[]) =>
  invoke<import("./types").EntryProgress[]>("entry_progress", { entryIds });

export const ocrStatus = () =>
  invoke<import("./types").OcrStatus>("ocr_status");
export const ocrSetup = (useGpu: boolean) =>
  invoke<import("./types").OcrStatus>("ocr_setup", { useGpu });
export const ocrRunPage = (entryId: string, index: number) =>
  invoke<import("./types").LocalOcrPage>("ocr_run_page", { entryId, index });
export const ocrResult = (entryId: string, index: number) =>
  invoke<import("./types").LocalOcrPage | null>("ocr_result", {
    entryId,
    index,
  });
export const ocrCancel = () => invoke<void>("ocr_cancel");
export const ocrPublishPage = (entryId: string, index: number) =>
  invoke<void>("ocr_publish_page", { entryId, index });
export async function onOcrProgress(
  handler: (progress: import("./types").OcrProgress) => void,
) {
  if (!isTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<import("./types").OcrProgress>("ocr-progress", (event) =>
    handler(event.payload),
  );
}

export const pageDraftNotes = (
  entryId: string,
  index: number,
  notes: Array<import("./types").JsonValue | null>,
) => invoke<Page>("page_draft_notes", { entryId, index, notes });
export const pageNoteDelete = (
  entryId: string,
  index: number,
  noteIndex: number,
) => invoke<Page>("page_note_delete", { entryId, index, noteIndex });
export const pageDraftTextAndNotes = (
  entryId: string,
  index: number,
  text: string,
  notes: Array<import("./types").JsonValue | null>,
) => invoke<Page>("page_draft", { entryId, index, text, notes });
export const historyOpen = (entryId: string, index: number) => invoke<void>("history_open", { entryId, index });
export const historyRecent = (limit = 8) => invoke<import("./types").RecentWork[]>("history_recent", { limit });
export const historyClear = () => invoke<void>("history_clear");

export const ocrDiagnostics = () => invoke<import("./types").OcrDiagnostics>("ocr_diagnostics");
export const ocrDoctor = () => invoke<string>("ocr_doctor");
export const ocrRepairModels = () => invoke<import("./types").OcrStatus>("ocr_repair_models");
export const editingPages = () => invoke<Page[]>("editing_pages");
export const projectPageActivity = (projectId: string) => invoke<Record<string, string>>("project_page_activity", { projectId });
export interface ReadRegion {
  kind: "projects" | "project" | "collections" | "collection" | "entry" | "pages" | "entries" | "collectionProgress" | "entryProgress";
  id?: string;
  ids?: string[];
}
export const regionCached = <T>(resource: ReadRegion) => invoke<T | null>("region_cached", { resource });
export const regionRefresh = <T>(resource: ReadRegion) => invoke<T>("region_refresh", { resource });
export async function searchStatus(): Promise<import("./types").SearchStatus> {
  if (isTauri()) return invoke("search_status");
  return { present: true, commit: null, page_count: 0, last_build: null, size: 0 };
}
export const searchChooseDump = () => invoke<string | null>("search_choose_dump");
export const searchBuild = (dumpPath?: string, cloneDump = false) =>
  invoke<import("./types").SearchStatus>("search_build", { dumpPath, cloneDump });
export const searchSync = () => invoke<number>("search_sync");
export async function searchQuery(query: import("./types").SearchQuery): Promise<import("./types").SearchResults> {
  if (isTauri()) return invoke("search_query", { query });
  const { fixtureSearch } = await import("../../apps/client/src/dev/search");
  return fixtureSearch(query);
}
export async function onSearchProgress(handler: (progress: import("./types").SearchProgress) => void) {
  if (!isTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<import("./types").SearchProgress>("search-progress", (event) => handler(event.payload));
}
export async function onSearchError(handler: (error: import("./types").AppError) => void) {
  if (!isTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<import("./types").AppError>("search-error", (event) => handler(event.payload));
}

export const glyphAttestations = (character: string, project: string | null = null, limit = 100) =>
  invoke<import("./types").GlyphAttestations>("glyph_attestations", { character, project, limit });
export const clipsList = () => invoke<import("./types").Clip[]>("clips_list");
export const clipCreate = (input: import("./types").ClipInput) =>
  invoke<import("./types").Clip>("clip_create", { input });
export const clipDelete = (id: string) => invoke<void>("clip_delete", { id });
export const glyphImageUrl = (infoUrl: string, url: string) =>
  invoke<string>("glyph_image_url", { infoUrl, url });
