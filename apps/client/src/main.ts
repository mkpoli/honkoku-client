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
    // Subscribe first so a link that arrives during startup is not lost; a
    // link delivered meanwhile is newer than the startup one.
    let received = false;
    await onOpenUrl((urls) => {
      received = true;
      follow(urls);
    });
    const initial = await getCurrent();
    if (!received) follow(initial);
  } catch {
    /* Without the link plugin the client still opens its home. */
  }
}
mount(App, { target });
