import type { SavedPage } from "@honkoku/client-api/types";
import { siteHref } from "./site";

/**
 * Points as the website's save-result dialog shows them: a review of someone
 * else's 添削希望 page counts tenfold, and a チェック adds 100.
 */
export function savePoints({
  count,
  isReview,
  isApproval,
}: Pick<SavedPage, "count" | "isReview" | "isApproval">) {
  return (isReview ? count * 10 : count) + (isApproval ? 100 : 0);
}

export function saveBonus({
  isReview,
  isApproval,
}: Pick<SavedPage, "isReview" | "isApproval">) {
  if (isReview && isApproval) return "添削・チェック";
  if (isApproval) return "チェック";
  if (isReview) return "添削";
}

/** The website's share text, pointing at the same コマ on the website. */
export function shareUrl(
  entryLabel: string,
  entryId: string,
  pageIndex: number,
  count: number,
) {
  const text = `『${entryLabel}』コマ${pageIndex + 1}を${count}文字翻刻しました！ @CloudHonkoku ${siteHref({ entryId, pageIndex })} #みんなで翻刻`;
  return `https://twitter.com/intent/tweet?text=${encodeURIComponent(text)}`;
}
