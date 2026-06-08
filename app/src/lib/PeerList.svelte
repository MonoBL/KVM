<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  interface PeerDto {
    id: number;
    name: string;
    host: string;
    port: number;
  }
  export let peers: PeerDto[] = [];

  // Track per-peer connection state.
  let connecting: Set<number> = new Set();
  let connected: Set<number> = new Set();
  let errors: Map<number, string> = new Map();

  async function connect(peer: PeerDto) {
    if (connected.has(peer.id) || connecting.has(peer.id)) return;
    connecting = new Set([...connecting, peer.id]);
    errors = new Map([...errors].filter(([k]) => k !== peer.id));
    try {
      await invoke("connect_to_peer", { host: peer.host, port: peer.port });
      connected = new Set([...connected, peer.id]);
    } catch (e: unknown) {
      errors = new Map([...errors, [peer.id, String(e)]]);
    } finally {
      connecting = new Set([...connecting].filter((id) => id !== peer.id));
    }
  }
</script>

<section>
  <h2>Discovered Peers</h2>
  {#if peers.length === 0}
    <p class="empty">No peers found. Start the engine and make sure other Hopper instances are running on the same network.</p>
  {:else}
    <ul class="peer-list">
      {#each peers as peer (peer.id)}
        <li class="peer-card">
          <span class="peer-icon">🖥</span>
          <div class="peer-info">
            <strong>{peer.name}</strong>
            <span class="peer-addr">{peer.host}:{peer.port}</span>
            {#if errors.has(peer.id)}
              <span class="peer-error">{errors.get(peer.id)}</span>
            {/if}
          </div>
          {#if connected.has(peer.id)}
            <span class="peer-status connected">Connected</span>
          {:else}
            <button
              class="btn-connect"
              disabled={connecting.has(peer.id)}
              on:click={() => connect(peer)}
            >
              {connecting.has(peer.id) ? "Connecting…" : "Connect"}
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  h2 {
    margin: 0 0 12px;
    font-size: 0.9rem;
    color: #aaa;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .empty {
    color: #666;
    font-size: 0.9rem;
    line-height: 1.5;
  }
  .peer-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .peer-card {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    background: #1a1a2e;
    border: 1px solid #2a2a4a;
    border-radius: 6px;
  }
  .peer-icon {
    font-size: 1.4rem;
  }
  .peer-info {
    flex: 1;
    display: flex;
    flex-direction: column;
  }
  .peer-info strong {
    font-size: 0.9rem;
  }
  .peer-addr {
    font-size: 0.78rem;
    color: #888;
    margin-top: 2px;
  }
  .peer-status {
    font-size: 0.75rem;
    padding: 2px 8px;
    border-radius: 10px;
    background: #1b4332;
    color: #4caf50;
  }
  .peer-status.connected {
    background: #1a3a5c;
    color: #4fc3f7;
  }
  .peer-error {
    font-size: 0.72rem;
    color: #ef5350;
    margin-top: 2px;
  }
  .btn-connect {
    padding: 4px 12px;
    border: 1px solid #3a3a6a;
    border-radius: 4px;
    background: #1e1e3a;
    color: #a0a0d0;
    cursor: pointer;
    font-size: 0.78rem;
    white-space: nowrap;
  }
  .btn-connect:hover:not(:disabled) {
    background: #2a2a5a;
    color: #d0d0f0;
  }
  .btn-connect:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
