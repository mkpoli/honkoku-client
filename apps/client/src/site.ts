import type { Route } from "./routes";

/** Public faces of みんなで翻刻, opened in the system browser. */
export const siteOrigin = "https://app.honkoku.org";
export const greetingUrl = `${siteOrigin}/locales/markdowns/greeting_ja.md`;
export const wikiUrl = "https://wiki.honkoku.org/doku.php?id=start";
export const learnUrl = "https://kula-kuzushiji.web.app/learn";

const segment = (value: string) => encodeURIComponent(value);

/**
 * The website URL for the same screen. Page numbers there are one-based
 * (`index + 1`), matching the コマN labels and the site's own link builders.
 * Screens the website does not have return undefined.
 */
export function siteHref(route: Route): string | undefined {
  if (route.editorSpike || route.markupHelp || route.glyph || route.clips)
    return;
  if (route.search)
    return `${siteOrigin}/search?keyword=${encodeURIComponent(route.query ?? "")}`;
  if (route.guidelines && route.projectId)
    return `${siteOrigin}/projects/${segment(route.projectId)}/guidelines`;
  if (route.entryId) {
    const page = route.pageIndex === undefined ? 1 : route.pageIndex + 1;
    return `${siteOrigin}/transcription/${segment(route.entryId)}/${page}`;
  }
  if (route.projectId)
    return `${siteOrigin}/projects/${segment(route.projectId)}${
      route.collectionId ? `/collections/${segment(route.collectionId)}` : ""
    }`;
  return `${siteOrigin}/`;
}

export function siteOpenLabel(route: Route): string {
  if (route.search) return "サイトで検索";
  if (route.entryId && route.pageIndex !== undefined) return "このコマをサイトで開く";
  if (route.entryId) return "この資料をサイトで開く";
  if (route.guidelines) return "このガイドラインをサイトで開く";
  if (route.collectionId) return "このコレクションをサイトで開く";
  if (route.projectId) return "このプロジェクトをサイトで開く";
  return "サイトのホームを開く";
}
