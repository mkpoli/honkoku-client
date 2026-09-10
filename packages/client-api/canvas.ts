import type { Canvas } from "./types";
/** Older entry documents retain Presentation 2 canvases verbatim. */
export function normalizeCanvas(value: unknown): Canvas {
  const c = value as Record<string, unknown>;
  const resource =
    (c.images as { resource?: Record<string, unknown> }[] | undefined)?.[0]
      ?.resource ?? {};
  const service = (
    Array.isArray(resource.service) ? resource.service[0] : resource.service
  ) as Record<string, unknown> | undefined;
  const serviceId = service?.["@id"] ?? service?.id;
  const thumb = c.thumbnail as Record<string, unknown> | string | undefined;
  const text = (v: unknown) => (typeof v === "string" ? v : null);
  return {
    ...c,
    id: text(c.id ?? c["@id"]) ?? "",
    width: Number(c.width ?? 0),
    height: Number(c.height ?? 0),
    infoJsonUrl:
      text(c.infoJsonUrl) ??
      (typeof serviceId === "string"
        ? `${serviceId.replace(/\/$/, "")}/info.json`
        : null),
    imageUrl: text(c.imageUrl ?? resource["@id"] ?? resource.id),
    thumbnailUrl:
      text(
        c.thumbnailUrl ??
          (typeof thumb === "string" ? thumb : (thumb?.["@id"] ?? thumb?.id)),
      ) ?? text(c.imageUrl ?? resource["@id"] ?? resource.id),
  };
}
