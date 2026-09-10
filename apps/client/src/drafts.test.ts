import { expect, test } from "bun:test";
import { Drafts } from "./drafts";

test("drafts coalesce and serialize requests; final flush sends the newest text", async () => {
  const sent: string[] = [];
  let release!: () => void;
  const queue = new Drafts(
    async (text) => {
      sent.push(text);
      if (text === "first")
        await new Promise<void>((resolve) => {
          release = resolve;
        });
    },
    () => {},
    10,
  );
  queue.request("discarded");
  queue.request("first");
  await Bun.sleep(20);
  queue.request("old");
  queue.request("newest");
  expect(sent).toEqual(["first"]);
  const flush = queue.flush();
  release();
  await flush;
  expect(sent).toEqual(["first", "newest"]);
  await queue.stop();
});

test("a failed draft remains pending and retries without losing a newer edit", async () => {
  let fail = true;
  const sent: string[] = [];
  const queue = new Drafts(
    async (text) => {
      if (fail) {
        fail = false;
        throw Error("offline");
      }
      sent.push(text);
    },
    () => {},
    10,
  );
  queue.request("retained");
  await expect(queue.flush()).rejects.toThrow("offline");
  await queue.flush();
  expect(sent).toEqual(["retained"]);
  queue.request("cancelled");
  await queue.stop();
  await Bun.sleep(20);
  expect(sent).toEqual(["retained"]);
});

test("autosave writes are separated by the interval under continuous typing", async () => {
  const times: number[] = [];
  const queue = new Drafts(
    async () => {
      times.push(Date.now());
    },
    () => {},
    30,
  );
  for (let i = 0; i < 15; i++) {
    queue.request(String(i));
    await Bun.sleep(5);
  }
  await Bun.sleep(40);
  await queue.stop();
  expect(times.length).toBeGreaterThanOrEqual(2);
  for (let i = 1; i < times.length; i++)
    expect(times[i] - times[i - 1]).toBeGreaterThanOrEqual(29);
});
