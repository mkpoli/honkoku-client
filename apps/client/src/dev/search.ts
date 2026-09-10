import fixture from "../../../../fixtures/home/search-kwic.json";
import type { SearchQuery, SearchResults } from "@honkoku/client-api/types";
export function fixtureSearch(query: SearchQuery): SearchResults {
  if (!query.text.trim())
    return { total: 0, hits: [], facets: [], next_cursor: null };
  if (query.text.replace(/\s/g, "") !== "蝦夷")
    throw {
      kind: "fixture",
      message:
        "ブラウザーでは「蝦夷」の検索結果を閲覧できます。ほかの語句はデスクトップアプリで検索してください。",
    };
  const data = fixture as SearchResults;
  const hits = data.hits.filter(
    (hit) =>
      (!query.project || hit.project_id === query.project) &&
      (!query.entry || hit.entry_id === query.entry),
  );
  const after = hits.filter(
    (hit) => !query.cursor || hit.page_id > query.cursor,
  );
  const slice = after.slice(0, query.limit);
  return {
    total: query.project || query.entry ? hits.length : data.total,
    hits: slice,
    facets: data.facets,
    next_cursor: after.length > slice.length ? slice.at(-1)!.page_id : null,
  };
}
