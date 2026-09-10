<script lang="ts">
  import { onMount } from "svelte";
  import { sessionCurrent } from "@honkoku/client-api/invoke";
  import type { CredentialStore, SignInProvider } from "@honkoku/client-api/types";
  let {
    mode = "signin",
    busy = false,
    error = "",
    onprovider,
    onimport,
    onlogout,
    onclose,
  }: {
    mode?: "signin" | "signout";
    busy?: boolean;
    error?: string;
    onprovider: (provider: SignInProvider) => void;
    onimport: () => void;
    onlogout: (clearSiteData: boolean) => void;
    onclose: () => void;
  } = $props();
  let dialog: HTMLDialogElement;
  let clearSiteData = $state(false);
  let credentialStore = $state<CredentialStore>();
  onMount(() => {
    dialog.showModal();
    if (mode === "signout") void sessionCurrent().then((session) => {
      credentialStore = session?.credential_store;
    }).catch(() => {});
  });
</script>

<dialog bind:this={dialog} {onclose} aria-labelledby="signin-title">
  <div class="heading">
    <h2 id="signin-title">{mode === "signin" ? "ログイン" : "ログアウト"}</h2>
    <button class="close" aria-label="閉じる" onclick={() => dialog.close()}
      >×</button
    >
  </div>
  {#if mode === "signin"}
    <div class="providers" aria-busy={busy}>
      <button
        class="primary"
        disabled={busy}
        onclick={() => onprovider("google.com")}>Googleでログイン</button
      >
      <p class="caption">Googleは埋め込みブラウザーでのログインを拒否することがあります。その場合はXでログインしてください。</p>
      <button disabled={busy} onclick={() => onprovider("twitter.com")}
        >Xでログイン</button
      >
    </div>
    {#if busy}<p role="status">
        ログインウィンドウで操作を続けてください。
      </p>{/if}
    <button class="import" disabled={busy} onclick={onimport}
      >開発用セッションを読み込む</button
    >
  {:else}
    {#if credentialStore}<p role="status">{credentialStore === "os" ? "資格情報はOSに保存" : "ファイルに保存"}</p>{/if}
    <label
      ><input
        type="checkbox"
        bind:checked={clearSiteData}
        disabled={busy}
      />サイトからもログアウト</label
    >
    <div class="actions">
      <button disabled={busy} onclick={() => dialog.close()}>キャンセル</button>
      <button
        class="primary"
        disabled={busy}
        onclick={() => onlogout(clearSiteData)}>ログアウト</button
      >
    </div>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</dialog>

<style>
  dialog {
    width: min(360px, calc(100vw - 48px));
    padding: 24px;
    border: 1px solid var(--border);
    border-radius: 12px;
    color: var(--text);
    background: var(--surface);
    box-shadow: 0 16px 64px #0003;
  }
  dialog::backdrop {
    background: #0005;
  }
  .heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 24px;
  }
  h2 {
    font-size: 20px;
    margin: 0;
  }
  .close {
    border: 0;
    background: transparent;
    font-size: 24px;
    padding: 0 8px;
  }
  .providers {
    display: grid;
    gap: 12px;
  }
  .providers button {
    padding: 12px 16px;
  }
  .caption {
    margin: 0;
    color: var(--text-muted);
  }
  .import {
    display: block;
    border: 0;
    background: transparent;
    text-decoration: underline;
    margin: 20px auto 0;
    font-size: 13px;
  }
  p {
    font-size: 13px;
    line-height: 1.7;
  }
  label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 14px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 12px;
    margin-top: 24px;
  }
  .actions button {
    padding: 8px 16px;
  }
</style>
