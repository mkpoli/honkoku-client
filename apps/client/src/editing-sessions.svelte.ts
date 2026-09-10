import type { Page } from "@honkoku/client-api/types";
class OpenSessions {
  pages = $state<Record<string, Page>>({});
  observe(page: Page) {
    if (page.status === "editing") this.pages[page.id] = page;
    else delete this.pages[page.id];
  }
}
export const openSessions = new OpenSessions();
