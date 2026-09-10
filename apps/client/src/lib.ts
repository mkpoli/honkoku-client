import type {
  Entry,
  Label,
  Page,
  PageNote,
  PageStatus,
  TimelineItem,
  User,
} from "@honkoku/client-api/types";
import { getUser } from "@honkoku/client-api/invoke";
export const label = (v?: Label | null): string =>
  typeof v === "string"
    ? v
    : v
      ? (v.ja ?? v.none ?? v.en ?? Object.values(v)[0] ?? []).join("／")
      : "";
export const number = (n?: number | null) =>
  n == null ? "—" : n.toLocaleString("ja-JP");
export const percent = (done = 0, total = 0) =>
  total > 0 ? Math.min(100, Math.max(0, (done / total) * 100)) : 0;
export const statuses: Record<string, { label: string; symbol: string }> = {
  default: { label: "未着手", symbol: "○" },
  initiated: { label: "翻刻中", symbol: "◐" },
  editing: { label: "編集中", symbol: "✎" },
  completed: { label: "完了", symbol: "✓" },
  frozen: { label: "凍結", symbol: "◇" },
};
export const status = (s: PageStatus = "default") =>
  statuses[s] ?? { label: s, symbol: "？" };
export const statusClass = (s: PageStatus = "default") =>
  s in statuses ? s : "default";
export const date = (value?: string | null) =>
  value && Number.isFinite(Date.parse(value))
    ? new Intl.DateTimeFormat("ja-JP", {
        month: "long",
        day: "numeric",
      }).format(new Date(value))
    : "日付不明";
export function relative(value: string, now = Date.now()): string {
  const ms = Math.max(0, now - Date.parse(value));
  if (ms < 60000) return "たった今";
  if (ms < 3600000) return `約${Math.floor(ms / 60000)}分前`;
  if (ms < 86400000) return `約${Math.floor(ms / 3600000)}時間前`;
  if (
    new Date(now).toDateString() ===
    new Date(Date.parse(value) + 86400000).toDateString()
  )
    return "昨日";
  return date(value);
}
export function action(item: TimelineItem): string {
  if (item.event.isApproval) return "レビューを承認";
  if (
    item.event.data &&
    typeof item.event.data === "object" &&
    !Array.isArray(item.event.data) &&
    item.event.data.status === "completed"
  )
    return `コマ${item.event.index + 1}を完了`;
  return item.event.count > 0
    ? `${number(item.event.count)}字を翻刻`
    : "翻刻を更新";
}
export function aggregate(
  items: TimelineItem[],
  hours = Infinity,
  now = Date.now(),
): { user: User; count: number }[] {
  const users = new Map<string, { user: User; count: number }>();
  for (const item of items) {
    const age = now - Date.parse(item.event.createdAt);
    if (age < 0 || age >= hours * 3600000 || item.event.count <= 0) continue;
    const record = users.get(item.event.uid) ?? {
      user: item.actor ?? { uid: item.event.uid, displayName: "名前不明" },
      count: 0,
    };
    record.count += item.event.count;
    users.set(item.event.uid, record);
  }
  return [...users.values()].sort((a, b) => b.count - a.count);
}
export const entryCounts = (entry: Entry) => ({
  default: Math.max(
    0,
    (entry.size ?? 0) - (entry.initiated ?? 0) - (entry.progress ?? 0),
  ),
  initiated: entry.initiated ?? 0,
  completed: entry.progress ?? 0,
});
export function allPages(entry: Entry, pages: Page[]): Page[] {
  const byIndex = new Map(pages.map((p) => [p.index, p]));
  return Array.from(
    { length: entry.size ?? entry.canvases?.length ?? 0 },
    (_, index) =>
      byIndex.get(index) ?? {
        id: `${entry.id}_${index}`,
        entryId: entry.id,
        index,
        status: "default",
        text: "",
        notes: [],
      },
  );
}
export function notes(page: Page): (PageNote | null)[] {
  return page.notes.map((n) =>
    n &&
    typeof n === "object" &&
    !Array.isArray(n) &&
    typeof n.content === "string"
      ? (n as unknown as PageNote)
      : null,
  );
}
const users = new Map<string, Promise<User>>();
export function user(uid: string) {
  if (!users.has(uid))
    users.set(
      uid,
      getUser(uid).catch((error) => {
        users.delete(uid);
        throw error;
      }),
    );
  return users.get(uid)!;
}
export function errorMessage(error: unknown): string {
  return error && typeof error === "object" && "message" in error
    ? String(error.message).replace(/\/home\/[^/\s]+/g, "~")
    : "読み込めませんでした。接続を確認して再試行してください。";
}
export function safeUrl(value?: string | null): string | undefined {
  try {
    const url = new URL(value ?? "");
    return ["https:", "http:"].includes(url.protocol) ? url.href : undefined;
  } catch {
    return;
  }
}
