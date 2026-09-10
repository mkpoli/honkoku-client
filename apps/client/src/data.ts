import { getEntry, isTauri } from "@honkoku/client-api/invoke";
import { iiifLocalUrl, iiifPrepareEntry } from "@honkoku/client-api/iiif";
import type { Canvas, Entry } from "@honkoku/client-api/types";
const entries = new Map<string, Promise<Entry>>();
const canvases = new Map<string, Promise<Canvas[]>>();
export function entryData(id: string): Promise<Entry> {
  if (!entries.has(id))
    entries.set(
      id,
      getEntry(id).catch((error) => {
        entries.delete(id);
        throw error;
      }),
    );
  return entries.get(id)!;
}
export function entryCanvases(id: string): Promise<Canvas[]> {
  if (!canvases.has(id))
    canvases.set(
      id,
      (async () => {
        if (!isTauri()) return (await entryData(id)).canvases ?? [];
        try {
          return (await iiifPrepareEntry(id)).map((c) => ({ ...c }));
        } catch {
          const original = (await entryData(id)).canvases ?? [];
          const result: Canvas[] = [];
          for (const c of original)
            result.push({
              ...c,
              infoJsonUrl: c.infoJsonUrl
                ? await iiifLocalUrl(c.infoJsonUrl)
                : null,
              imageUrl: c.imageUrl ? await iiifLocalUrl(c.imageUrl) : null,
              thumbnailUrl: c.thumbnailUrl
                ? await iiifLocalUrl(c.thumbnailUrl)
                : null,
            });
          return result;
        }
      })().catch((error) => {
        canvases.delete(id);
        throw error;
      }),
    );
  return canvases.get(id)!;
}
