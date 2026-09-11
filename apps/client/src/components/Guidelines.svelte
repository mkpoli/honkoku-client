<script lang="ts">
  import type { Project } from "@honkoku/client-api/types";
  import Markdown from "./Markdown.svelte";
  import standard from "../standard-guidelines.md?raw";
  let { project }: { project: Project } = $props();
</script>

<article class="panel guidelines reference-page">
  <h1>翻刻ガイドライン</h1>
  <p>{project.title}</p>
  {#if !project.useOwnGuidelines}<section aria-label="標準ガイドライン">
      <Markdown text={standard} />
    </section>{/if}
  {#if project.guidelines}<section aria-label="プロジェクト固有ガイドライン">
      <h2>プロジェクト固有ガイドライン</h2>
      <Markdown text={project.guidelines} />
    </section>{/if}
  {#if project.useOwnGuidelines && !project.guidelines}<p>
      このプロジェクトにはガイドラインが設定されていません。
    </p>{/if}
</article>
