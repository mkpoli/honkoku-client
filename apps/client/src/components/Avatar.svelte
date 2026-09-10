<script lang="ts">
  import type { User } from "@honkoku/client-api/types";
  import { safeUrl } from "../lib";
  let { user, small = false }: { user?: User | null; small?: boolean } =
    $props();
  let failed = $state(false);
  $effect(() => {
    user?.photoURL;
    failed = false;
  });
</script>

<span class="avatar" class:small aria-hidden="true">
  {#if safeUrl(user?.photoURL) && !failed}<img
      src={user!.photoURL!}
      alt=""
      loading="lazy"
      referrerpolicy="no-referrer"
      onerror={() => (failed = true)}
    />{:else}{[...(user?.displayName ?? "？")][0]}{/if}
</span>
