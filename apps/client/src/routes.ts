export interface Route { projectId?: string; collectionId?: string; entryId?: string; pageId?: string }
export function parseRoute(hash: string): Route {
  const parts = hash.replace(/^#\/?/, '').split('/');
  const route: Route = {};
  const keys = ['projects', 'collections', 'entries', 'pages'];
  const fields = ['projectId', 'collectionId', 'entryId', 'pageId'] as const;
  for (let i = 0; i < keys.length; i++) {
    if (parts[i * 2] !== keys[i] || !parts[i * 2 + 1]) break;
    try { route[fields[i]] = decodeURIComponent(parts[i * 2 + 1]); } catch { break; }
  }
  return route;
}
export function href(route: Route): string {
  let path = '#';
  for (const [name, id] of [['projects', route.projectId], ['collections', route.collectionId], ['entries', route.entryId], ['pages', route.pageId]]) {
    if (!id) break;
    path += `/${name}/${encodeURIComponent(id)}`;
  }
  return path === '#' ? '#/' : path;
}
