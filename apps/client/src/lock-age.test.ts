import { expect, test } from "bun:test";
import { lockAge } from "./lock-age";

const updatedAt = "2026-09-28T03:00:00.000Z";
const touched = Date.parse(updatedAt);
const day = 24 * 60 * 60 * 1000;

test("age measures the time since the last draft write", () => {
  expect(lockAge(updatedAt, touched + 180000)).toEqual({
    ageMs: 180000,
    idle: false,
  });
  expect(lockAge(updatedAt, touched)).toEqual({ ageMs: 0, idle: false });
});

test("a lock is idle only after more than 24 hours", () => {
  for (const ageMs of [day - 1, day, day + 1, 30 * day]) {
    expect(lockAge(updatedAt, touched + ageMs)).toEqual({
      ageMs,
      idle: ageMs > day,
    });
  }
});

test("a new draft resets an idle lock's age", () => {
  const now = touched + 2 * day;
  expect(lockAge(updatedAt, now).idle).toBe(true);
  expect(lockAge(new Date(now - 3000).toISOString(), now)).toEqual({
    ageMs: 3000,
    idle: false,
  });
});

test("missing or invalid timestamps have unknown age and are never idle", () => {
  for (const value of [undefined, null, "", "invalid"]) {
    expect(lockAge(value, touched)).toEqual({ ageMs: null, idle: false });
  }
});

test("future timestamps are clamped to zero for clock skew", () => {
  expect(lockAge(updatedAt, touched - 60000)).toEqual({
    ageMs: 0,
    idle: false,
  });
});

test("Firestore timestamps with fractional seconds retain millisecond age", () => {
  expect(lockAge("2026-09-28T03:00:00.693887000Z", touched + 1000)).toEqual({
    ageMs: 307,
    idle: false,
  });
});
