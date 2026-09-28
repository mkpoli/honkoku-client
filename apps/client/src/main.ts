import "./theme";
import "@honkoku/ui/tokens.css";
import "./style.css";
import { isTauri } from "@honkoku/client-api/invoke";
import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { mount } from "svelte";
import App from "./App.svelte";
import { href } from "./routes";
import { siteRoute } from "./site";
const target = document.getElementById("app");
if (!target) throw new Error("Application mount point is missing");

/** Shows the screen for the last website link among `urls`, if any. */
function follow(urls: string[] | null) {
  const route = urls
    ?.map(siteRoute)
    .filter((r) => r !== undefined)
    .at(-1);
  if (route) location.hash = href(route);
}

if (isTauri()) {
  try {
    follow(await getCurrent());
    await onOpenUrl(follow);
  } catch {
    /* Without the link plugin the client still opens its home. */
  }
}
mount(App, { target });
