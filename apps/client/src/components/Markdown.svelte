<script lang="ts">
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  import { isTauri } from "@honkoku/client-api/invoke";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { safeUrl } from "../lib";
  let { text }: { text: string } = $props();
  let html = $derived(
    DOMPurify.sanitize(marked.parse(text, { async: false }), {
      USE_PROFILES: { html: true },
      FORBID_TAGS: ["style", "input", "form", "button"],
      FORBID_ATTR: ["style"],
    }),
  );
  let error = $state("");
  function links(element: HTMLElement) {
    const handle = (event: MouseEvent) => {
      const anchor = (event.target as HTMLElement).closest("a");
      if (!anchor) return;
      const url = safeUrl(anchor.href);
      if (!url) {
        event.preventDefault();
        return;
      }
      if (isTauri()) {
        event.preventDefault();
        void openUrl(url).catch(
          () => (error = "ブラウザーを開けませんでした。"),
        );
      } else {
        anchor.target = "_blank";
        anchor.rel = "noopener noreferrer";
      }
    };
    element.addEventListener("click", handle);
    return { destroy: () => element.removeEventListener("click", handle) };
  }
</script>

<div class="markdown" use:links>{@html html}</div>
{#if error}<p role="alert" class="error">{error}</p>{/if}
