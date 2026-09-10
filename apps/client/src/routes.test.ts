import { expect, test } from "bun:test";
import { href, parseRoute } from "./routes";
test("all public routes round trip including page zero", () => {
  for (const route of [
    {},
    { projectId: "ainu" },
    { projectId: "ainu", collectionId: "資料" },
    { entryId: "entry" },
    { entryId: "entry", pageIndex: 0 },
    { entryId: "entry", pageIndex: 17 },
    { entryId: "entry", pageIndex: 3, column: 0 },
    { entryId: "entry", pageIndex: 3, column: 12 },
    { search: true, query: "蝦夷 & 𛀁?" },
  ])
    expect(parseRoute(href(route))).toEqual(route);
});
test("malformed routes cannot silently become home or a different page", () => {
  for (const route of [
    "#/entries/e/pages/-1",
    "#/entries/e/pages/1.2",
    "#/projects/%zz",
    "#/projects/a/unknown/b",
    "#/entries/e/pages/999999999999999999",
    "#/entries/e/pages/1?column=-1",
    "#/entries/e/pages/1?column=1.2",
    "#/entries/e/pages/1?column=999999999999999999",
  ])
    expect(parseRoute(route).invalid).toBe(true);
});
