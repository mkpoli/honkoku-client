import { expect, test } from "bun:test";
import { normalizeCanvas } from "./canvas";
test("raw Presentation 2 canvas supplies upstream image and service URLs", () => {
  const c = normalizeCanvas({
    "@id": "canvas/1",
    width: 100,
    height: 200,
    images: [
      {
        resource: {
          "@id": "https://image/full/full/0/default.jpg",
          service: [{ "@id": "https://image" }],
        },
      },
    ],
    thumbnail: { "@id": "https://thumb" },
  });
  expect(c.id).toBe("canvas/1");
  expect(c.infoJsonUrl).toBe("https://image/info.json");
  expect(c.thumbnailUrl).toBe("https://thumb");
});
test("normalized canvas URLs are preserved", () => {
  const c = {
    id: "1",
    width: 1,
    height: 2,
    imageUrl: "https://image",
    infoJsonUrl: "https://info",
    thumbnailUrl: "https://thumb",
  };
  expect(normalizeCanvas(c)).toEqual(c);
});
