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
