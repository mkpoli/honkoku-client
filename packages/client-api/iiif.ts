import { invoke } from "./invoke";

export interface PreparedCanvas {
  id: string;
  width: number;
  height: number;
  infoJsonUrl: string | null;
  imageUrl: string | null;
  thumbnailUrl: string | null;
}

/** Compute a protocol URL. Use iiifLocalUrl to register a newly encountered host. */
export function localUrl(upstream: string): string {
  const isWindows =
    typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);
  const origin = isWindows
    ? "http://honkoku-iiif.localhost"
    : "honkoku-iiif://localhost";
  return `${origin}/fetch?url=${encodeURIComponent(upstream)}`;
}

export const iiifPrepareEntry = (entryId: string) =>
  invoke<PreparedCanvas[]>("iiif_prepare_entry", { entryId });

export const iiifLocalUrl = (url: string) =>
  invoke<string>("iiif_local_url", { url });
