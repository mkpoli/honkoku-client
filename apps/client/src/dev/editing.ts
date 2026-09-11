import type {
  Entry,
  Page,
  SaveOptions,
  TimelineItem,
  User,
} from "@honkoku/client-api/types";

// Count inserted characters with a character diff, including replacements.
export function addedCount(before: string, after: string): number {
  const a = [...before],
    b = [...after];
  let prefix = 0;
  while (prefix < a.length && prefix < b.length && a[prefix] === b[prefix])
    prefix++;
  let endA = a.length,
    endB = b.length;
  while (endA > prefix && endB > prefix && a[endA - 1] === b[endB - 1]) {
    endA--;
    endB--;
  }
  const old = a.slice(prefix, endA),
    next = b.slice(prefix, endB);
  const rows = Array.from(
    { length: old.length + 1 },
    () => new Uint32Array(next.length + 1),
  );
  for (let i = old.length - 1; i >= 0; i--)
    for (let j = next.length - 1; j >= 0; j--)
      rows[i][j] =
        old[i] === next[j]
          ? rows[i + 1][j + 1] + 1
          : Math.max(rows[i + 1][j], rows[i][j + 1]);
  let i = 0,
    j = 0,
    count = 0;
  while (j < next.length) {
    if (i < old.length && old[i] === next[j]) {
      i++;
      j++;
    } else if (i < old.length && rows[i + 1][j] >= rows[i][j + 1]) i++;
    else {
      if (!/\s/u.test(next[j])) count++;
      j++;
    }
  }
  return count;
}

export function fixtureEdit(
  command: string,
  args: Record<string, unknown>,
  page: Page,
  actor: User | null,
  entry: Entry,
  events: TimelineItem[],
) {
  const mine = !!actor && page.status === "editing" && page.tempEditedBy === actor.uid;
  if (command === "page_lock_state")
    return {
      page: structuredClone(page),
      pageId: page.id,
      status: page.status,
      tempEditedBy: page.tempEditedBy ?? null,
      isMine: mine,
      syncMode: page.syncMode ?? false,
      updateTime: page.updatedAt ?? "",
    };
  if (!actor) throw { kind: "signedOut", message: "ログインしてください。" };
  if (command === "page_lock") {
    if (page.status === "editing")
      throw { kind: "conflict", message: "このコマは編集中です。" };
    Object.assign(page, {
      prevStatus: page.status,
      status: "editing",
      tempText: page.text,
      tempTextChanged: false,
      tempNotes: structuredClone(page.notes),
      tempEditedBy: actor.uid,
      syncMode: args.syncMode ?? false,
      updatedAt: new Date().toISOString(),
    });
  } else {
    if (!mine) throw { kind: "conflict", message: "編集状態が変わりました。" };
    if (command === "page_draft")
      Object.assign(page, {
        tempText: String(args.text),
        ...(Array.isArray(args.notes)
          ? { tempNotes: structuredClone(args.notes) }
          : {}),
        tempTextChanged: true,
        updatedAt: new Date().toISOString(),
      });
    if (command === "page_draft_notes")
      Object.assign(page, {
        tempNotes: structuredClone(args.notes),
        updatedAt: new Date().toISOString(),
      });
    if (command === "page_discard") {
      page.status = page.prevStatus ?? "default";
      return;
    }
    if (command === "page_save") {
      const options = args.options as SaveOptions;
      if (options.isApproval !== undefined) {
        const approvers=page.approvedBy ?? [];
        if (options.isApproval && !approvers.includes(actor.uid) && approvers.length >= 2) throw {kind:"invalid", message:"チェックできるのは2人までです。"};
        page.approvedBy=options.isApproval ? [...new Set([...approvers,actor.uid])] : approvers.filter(uid => uid !== actor.uid);
      }
      const count = addedCount(page.text, page.tempText ?? "");
      const isReview =
        page.requestReview === true && page.editedBy !== actor.uid;
      Object.assign(page, {
        text: page.tempText ?? "",
        notes: structuredClone(page.tempNotes ?? []),
        editedBy: actor.uid,
        status: options.status ?? "initiated",
        share: options.share ?? false,
        requestReview: options.requestReview ?? false,
        syncMode: false,
      });
      const id = crypto.randomUUID().replaceAll("-", "").slice(0, 20);
      events.unshift({
        event: {
          id,
          uid: actor.uid,
          projectId: entry.projectId,
          entryId: entry.id,
          transcriptionId: page.id,
          index: page.index,
          eventType: "transcription",
          count,
          isReview,
          ...options,
          createdAt: new Date().toISOString(),
          data: JSON.parse(JSON.stringify(page)),
        },
        actor,
        entryLabel: entry.label,
        projectTitle: null,
        excerpt: page.text,
      });
      return { page: structuredClone(page), timelineEventId: id, count };
    }
  }
  return structuredClone(page);
}
