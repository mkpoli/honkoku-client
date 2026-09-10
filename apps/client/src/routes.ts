export interface Route {
  projectId?: string;
  collectionId?: string;
  entryId?: string;
  pageIndex?: number;
  invalid?: boolean;
  editorSpike?: boolean;
}
export function parseRoute(hash: string): Route {
  try {
    const path = hash.replace(/^#/, "") || "/";
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
        ...(entry[2] === undefined ? {} : { pageIndex: Number(entry[2]) }),
      };
  } catch {
    /* Invalid percent encoding is an unknown route. */
  }
  return { invalid: true };
}
export function href(route: Route): string {
  if (route.entryId)
    return `#/entries/${encodeURIComponent(route.entryId)}${route.pageIndex === undefined ? "" : `/pages/${route.pageIndex}`}`;
  if (route.projectId)
    return `#/projects/${encodeURIComponent(route.projectId)}${route.collectionId ? `/collections/${encodeURIComponent(route.collectionId)}` : ""}`;
  return "#/";
}
