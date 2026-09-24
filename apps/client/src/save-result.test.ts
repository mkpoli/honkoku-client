import { expect, test } from "bun:test";
import { saveBonus, savePoints, shareUrl } from "./save-result";

test("points follow the website's save-result dialog", () => {
  const plain = { count: 12, isReview: false, isApproval: false };
  expect(savePoints(plain)).toBe(12);
  expect(savePoints({ ...plain, isReview: true })).toBe(120);
  expect(savePoints({ ...plain, isApproval: true })).toBe(112);
  expect(savePoints({ ...plain, isReview: true, isApproval: true })).toBe(220);
  expect(saveBonus(plain)).toBeUndefined();
  expect(saveBonus({ isReview: true, isApproval: false })).toBe("添削");
  expect(saveBonus({ isReview: false, isApproval: true })).toBe("チェック");
  expect(saveBonus({ isReview: true, isApproval: true })).toBe(
    "添削・チェック",
  );
});

test("the share text links the same コマ on the website", () => {
  const url = new URL(shareUrl("蝦夷方言藻汐草", "abc", 2, 34));
  expect(url.origin + url.pathname).toBe("https://twitter.com/intent/tweet");
  expect(url.searchParams.get("text")).toBe(
    "『蝦夷方言藻汐草』コマ3を34文字翻刻しました！ @CloudHonkoku https://app.honkoku.org/transcription/abc/3 #みんなで翻刻",
  );
});
