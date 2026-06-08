<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import LayoutEditor from "./lib/LayoutEditor.svelte";
  import PermissionPanel from "./lib/PermissionPanel.svelte";
  import PeerList from "./lib/PeerList.svelte";

  interface StatusDto {
    role: string;
    control: string;
    running: boolean;
  }

  interface PeerDto {
    id: number;
    name: string;
    host: string;
    port: number;
  }

  let status: StatusDto = { role: "server", control: "local", running: false };
  let peers: PeerDto[] = [];
  let activeTab: "peers" | "layout" | "permissions" = "peers";
  let roleSelect = "server";

  async function refresh() {
    status = await invoke<StatusDto>("get_status");
    peers = await invoke<PeerDto[]>("get_peers");
    roleSelect = status.role;
  }

  async function toggleEngine() {
    if (status.running) {
      await invoke("stop_engine");
    } else {
      await invoke("start_engine");
    }
    await refresh();
  }

  async function applyRole() {
    await invoke("set_role", { role: roleSelect });
    await refresh();
  }

  onMount(async () => {
    await refresh();
    await listen<PeerDto>("peer-discovered", (event) => {
      peers = [...peers, event.payload];
    });
    // Poll status every 2s
    setInterval(refresh, 2000);
  });
</script>

<div class="app">
  <header>
    <div class="brand">
      <span class="logo">⌨</span>
      <h1>Hopper KVM</h1>
    </div>
    <div class="status-bar">
      <span class="badge" class:active={status.running}>
        {status.running ? "Running" : "Stopped"}
      </span>
      <span class="control-label">
        Control: <strong>{status.control}</strong>
      </span>
    </div>
  </header>

  <div class="toolbar">
    <div class="role-row">
      <label>Role</label>
      <select bind:value={roleSelect} on:change={applyRole}>
        <option value="server">Server (captures input)</option>
        <option value="client">Client (receives input)</option>
      </select>
    </div>
    <button class="btn" class:danger={status.running} on:click={toggleEngine}>
      {status.running ? "Stop" : "Start"}
    </button>
  </div>

  <nav class="tabs">
    <button
      class:active={activeTab === "peers"}
      on:click={() => (activeTab = "peers")}
    >Peers ({peers.length})</button>
    <button
      class:active={activeTab === "layout"}
      on:click={() => (activeTab = "layout")}
    >Layout</button>
    <button
      class:active={activeTab === "permissions"}
      on:click={() => (activeTab = "permissions")}
    >Permissions</button>
  </nav>

  <main>
    {#if activeTab === "peers"}
      <PeerList {peers} />
    {:else if activeTab === "layout"}
      <LayoutEditor />
    {:else}
      <PermissionPanel />
    {/if}
  </main>
</div>

<style>
  :global(*, *::before, *::after) {
    box-sizing: border-box;
  }
  :global(body) {
    margin: 0;
    font-family: system-ui, -apple-system, sans-serif;
    background: #0f0f1a;
    color: #d0d0e8;
    font-size: 14px;
  }
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 20px;
    background: #1a1a2e;
    border-bottom: 1px solid #2a2a4a;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .logo {
    font-size: 1.4rem;
  }
  h1 {
    margin: 0;
    font-size: 1.1rem;
    color: #4fc3f7;
  }
  .status-bar {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .badge {
    padding: 2px 10px;
    border-radius: 12px;
    background: #2a2a3a;
    font-size: 0.8rem;
    color: #888;
  }
  .badge.active {
    background: #1b4332;
    color: #4caf50;
  }
  .control-label {
    font-size: 0.85rem;
    color: #aaa;
  }
  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 20px;
    background: #151525;
    border-bottom: 1px solid #2a2a4a;
  }
  .role-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  label {
    color: #aaa;
    font-size: 0.85rem;
  }
  select {
    background: #1e1e32;
    color: #d0d0e8;
    border: 1px solid #3a3a5a;
    border-radius: 4px;
    padding: 4px 8px;
    font-size: 0.85rem;
  }
  .btn {
    padding: 6px 16px;
    border: none;
    border-radius: 4px;
    background: #1565c0;
    color: #fff;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .btn:hover {
    background: #1976d2;
  }
  .btn.danger {
    background: #b71c1c;
  }
  .btn.danger:hover {
    background: #c62828;
  }
  .tabs {
    display: flex;
    border-bottom: 1px solid #2a2a4a;
    background: #151525;
  }
  .tabs button {
    padding: 8px 20px;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: #888;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .tabs button.active {
    color: #4fc3f7;
    border-bottom-color: #4fc3f7;
  }
  main {
    flex: 1;
    overflow-y: auto;
    padding: 16px 20px;
  }
</style>
