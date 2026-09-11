export interface Route {
  markupHelp?: boolean;
  guidelines?: boolean;
  glyph?: string;
  clips?: boolean;
  search?: boolean;
  query?: string;
  column?: number;
  projectId?: string;
  collectionId?: string;
  entryId?: string;
  pageIndex?: number;
  invalid?: boolean;
  editorSpike?: boolean;
}
export function parseRoute(hash: string): Route {
  try {
    const raw = hash.replace(/^#/, "") || "/";
    const split = raw.indexOf("?");
    const path = split < 0 ? raw : raw.slice(0, split);
    const params = new URLSearchParams(split < 0 ? "" : raw.slice(split + 1));
    if (path === "/help/markup") return {markupHelp:true};
    const guidelines = /^\/projects\/([^/]+)\/guidelines\/?$/.exec(path);
    if(guidelines) return {projectId:decodeURIComponent(guidelines[1]),guidelines:true};
    if (path === "/clips") return { clips: true };
    const glyph = /^\/glyphs\/([^/]+)\/?$/.exec(path);
    if (glyph) return { glyph: decodeURIComponent(glyph[1]) };
    if (path === "/search")
      return { search: true, query: params.get("q") ?? "" };
    const columnText = params.get("column");
    if (
      columnText !== null &&
      (!/^\d+$/.test(columnText) || !Number.isSafeInteger(Number(columnText)))
    )
      return { invalid: true };
    const column = columnText === null ? undefined : Number(columnText);
    if (path === "/") return {};
    if (import.meta.env?.DEV && path === "/spike/editor")
      return { editorSpike: true };
    const project = /^\/projects\/([^/]+)(?:\/collections\/([^/]+))?\/?$/.exec(
      path,
    );
    if (project)
      return {
        projectId: decodeURIComponent(project[1]),
        ...(project[2] ? { collectionId: decodeURIComponent(project[2]) } : {}),
      };
    const entry = /^\/entries\/([^/]+)(?:\/pages\/(\d+))?\/?$/.exec(path);
    if (
      entry &&
      (entry[2] === undefined || Number.isSafeInteger(Number(entry[2])))
    )
      return {
        entryId: decodeURIComponent(entry[1]),
        ...(entry[2] === undefined
          ? {}
          : {
              pageIndex: Number(entry[2]),
              ...(column === undefined ? {} : { column }),
            }),
      };
  } catch {
    /* Invalid percent encoding is an unknown route. */
  }
  return { invalid: true };
}
export function href(route: Route): string {
  if(route.markupHelp) return "#/help/markup";
  if(route.guidelines && route.projectId) return `#/projects/${encodeURIComponent(route.projectId)}/guidelines`;
  if (route.search)
    return `#/search?q=${encodeURIComponent(route.query ?? "")}`;
  if (route.entryId)
    return `#/entries/${encodeURIComponent(route.entryId)}${route.pageIndex === undefined ? "" : `/pages/${route.pageIndex}${route.column === undefined ? "" : `?column=${route.column}`}`}`;
  if (route.projectId)
    return `#/projects/${encodeURIComponent(route.projectId)}${route.collectionId ? `/collections/${encodeURIComponent(route.collectionId)}` : ""}`;
  return "#/";
}
