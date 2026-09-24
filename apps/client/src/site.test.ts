import { expect, test } from "bun:test";
import { greetingUrl, learnUrl, siteHref, siteOpenLabel, wikiUrl } from "./site";

test("site URLs follow the website's route table", () => {
  expect(siteHref({})).toBe("https://app.honkoku.org/");
  expect(siteHref({ projectId: "ainu" })).toBe(
    "https://app.honkoku.org/projects/ainu",
  );
  expect(siteHref({ projectId: "ainu", collectionId: "資料" })).toBe(
    "https://app.honkoku.org/projects/ainu/collections/%E8%B3%87%E6%96%99",
  );
  expect(siteHref({ projectId: "ainu", guidelines: true })).toBe(
    "https://app.honkoku.org/projects/ainu/guidelines",
  );
  expect(siteHref({ search: true, query: "蝦夷 & 𛀁?" })).toBe(
    "https://app.honkoku.org/search?keyword=%E8%9D%A6%E5%A4%B7%20%26%20%F0%9B%80%81%3F",
  );
  expect(siteHref({ entryId: "entry" })).toBe(
    "https://app.honkoku.org/transcription/entry/1",
  );
});

test("page numbers in website URLs are one-based", () => {
  expect(siteHref({ entryId: "entry", pageIndex: 0 })).toBe(
    "https://app.honkoku.org/transcription/entry/1",
  );
  expect(siteHref({ entryId: "entry", pageIndex: 6 })).toBe(
    "https://app.honkoku.org/transcription/entry/7",
  );
  expect(siteHref({ entryId: "entry", pageIndex: 17, column: 3 })).toBe(
    "https://app.honkoku.org/transcription/entry/18",
  );
});

test("client-only screens have no website URL", () => {
  expect(siteHref({ markupHelp: true })).toBeUndefined();
  expect(siteHref({ glyph: "熙" })).toBeUndefined();
  expect(siteHref({ clips: true })).toBeUndefined();
  expect(siteHref({ editorSpike: true })).toBeUndefined();
});

test("open labels name the current screen", () => {
  expect(siteOpenLabel({ entryId: "entry", pageIndex: 6 })).toBe(
    "このコマをサイトで開く",
  );
  expect(siteOpenLabel({ entryId: "entry" })).toBe("この資料をサイトで開く");
  expect(siteOpenLabel({ projectId: "ainu" })).toBe(
    "このプロジェクトをサイトで開く",
  );
  expect(siteOpenLabel({})).toBe("サイトのホームを開く");
});

test("menu destinations match the website's own links", () => {
  expect(greetingUrl).toBe(
    "https://app.honkoku.org/locales/markdowns/greeting_ja.md",
  );
  expect(wikiUrl).toBe("https://wiki.honkoku.org/doku.php?id=start");
  expect(learnUrl).toBe("https://kula-kuzushiji.web.app/learn");
});
