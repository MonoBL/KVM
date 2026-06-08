<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  interface PermissionsDto {
    accessibility: boolean;
    input_monitoring: boolean;
  }

  let perms: PermissionsDto = { accessibility: false, input_monitoring: false };

  onMount(async () => {
    perms = await invoke<PermissionsDto>("check_permissions");
  });

  async function requestPerm(which: string) {
    await invoke("request_permission", { which });
    // Re-check after 2s (user may have just granted it)
    setTimeout(async () => {
      perms = await invoke<PermissionsDto>("check_permissions");
    }, 2000);
  }
</script>

<section>
  <h2>macOS Permissions</h2>
  <p class="hint">Hopper needs these to capture and inject keyboard/mouse input.</p>

  <ul class="perm-list">
    <li class="perm-item">
      <div class="perm-info">
        <strong>Accessibility</strong>
        <span>Required to inject keystrokes and mouse events (client role).</span>
      </div>
      <div class="perm-action">
        {#if perms.accessibility}
          <span class="badge ok">Granted</span>
        {:else}
          <span class="badge missing">Missing</span>
          <button class="btn" on:click={() => requestPerm("accessibility")}>
            Open Settings
          </button>
        {/if}
      </div>
    </li>

    <li class="perm-item">
      <div class="perm-info">
        <strong>Input Monitoring</strong>
        <span>Required to capture keyboard events without them appearing locally (server role).</span>
      </div>
      <div class="perm-action">
        {#if perms.input_monitoring}
          <span class="badge ok">Granted</span>
        {:else}
          <span class="badge missing">Missing</span>
          <button class="btn" on:click={() => requestPerm("input_monitoring")}>
            Open Settings
          </button>
        {/if}
      </div>
    </li>
  </ul>

  <p class="note">
    After granting permissions, restart Hopper. Unsigned dev builds may lose permissions
    after recompile — this is an Apple limitation. Signed distribution builds retain permissions.
  </p>
</section>

<style>
  h2 {
    margin: 0 0 6px;
    font-size: 0.9rem;
    color: #aaa;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .hint {
    color: #888;
    font-size: 0.85rem;
    margin: 0 0 16px;
  }
  .perm-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .perm-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    background: #1a1a2e;
    border: 1px solid #2a2a4a;
    border-radius: 6px;
    gap: 12px;
  }
  .perm-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .perm-info strong {
    font-size: 0.9rem;
  }
  .perm-info span {
    font-size: 0.78rem;
    color: #888;
  }
  .perm-action {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }
  .badge {
    padding: 2px 10px;
    border-radius: 10px;
    font-size: 0.75rem;
  }
  .badge.ok {
    background: #1b4332;
    color: #4caf50;
  }
  .badge.missing {
    background: #3a1a1a;
    color: #ef5350;
  }
  .btn {
    padding: 4px 12px;
    border: none;
    border-radius: 4px;
    background: #1565c0;
    color: #fff;
    cursor: pointer;
    font-size: 0.8rem;
  }
  .note {
    margin-top: 20px;
    font-size: 0.78rem;
    color: #666;
    line-height: 1.5;
    border-left: 3px solid #2a2a4a;
    padding-left: 12px;
  }
</style>
