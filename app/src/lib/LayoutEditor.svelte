<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  interface ScreenDto {
    id: number;
    name: string;
    width: number;
    height: number;
    col: number;
    row: number;
  }

  let screens: ScreenDto[] = [];
  let dragging: ScreenDto | null = null;
  let dragOffsetCol = 0;
  let dragOffsetRow = 0;
  let saved = false;

  const CELL = 80; // px per grid cell
  const COLS = 8;
  const ROWS = 4;

  onMount(async () => {
    screens = await invoke<ScreenDto[]>("get_screens");
  });

  function startDrag(e: MouseEvent, screen: ScreenDto) {
    dragging = screen;
    e.preventDefault();
  }

  function onGridMouseUp(e: MouseEvent, col: number, row: number) {
    if (!dragging) return;
    // Prevent two screens on same cell
    const conflict = screens.find(
      (s) => s !== dragging && s.col === col && s.row === row
    );
    if (!conflict) {
      dragging.col = col;
      dragging.row = row;
      screens = [...screens];
    }
    dragging = null;
  }

  async function saveLayout() {
    await invoke("set_layout", { screens });
    saved = true;
    setTimeout(() => (saved = false), 1500);
  }

  function screenStyle(s: ScreenDto) {
    return `left:${s.col * CELL}px;top:${s.row * CELL}px;width:${CELL - 4}px;height:${Math.round(CELL * (s.height / s.width)) - 4}px`;
  }
</script>

<section>
  <div class="header">
    <h2>Screen Layout</h2>
    <button class="btn" on:click={saveLayout}>{saved ? "Saved!" : "Save Layout"}</button>
  </div>
  <p class="hint">Drag screens to arrange. The cursor switches machine when it crosses an edge.</p>

  <div
    class="grid"
    style="width:{COLS * CELL}px;height:{ROWS * CELL}px"
    on:mouseleave={() => (dragging = null)}
  >
    <!-- Grid cells (drop targets) -->
    {#each Array(ROWS) as _, r}
      {#each Array(COLS) as _, c}
        <div
          class="cell"
          class:highlight={dragging !== null}
          style="left:{c * CELL}px;top:{r * CELL}px;width:{CELL}px;height:{CELL}px"
          on:mouseup={(e) => onGridMouseUp(e, c, r)}
        ></div>
      {/each}
    {/each}

    <!-- Screen tiles -->
    {#each screens as screen (screen.id)}
      <div
        class="screen-tile"
        class:local={screen.col === 0 && screen.row === 0}
        class:is-dragging={dragging === screen}
        style={screenStyle(screen)}
        on:mousedown={(e) => startDrag(e, screen)}
      >
        <span class="screen-name">{screen.name}</span>
        <span class="screen-res">{screen.width}×{screen.height}</span>
      </div>
    {/each}
  </div>
</section>

<style>
  h2 {
    margin: 0;
    font-size: 0.9rem;
    color: #aaa;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  .hint {
    color: #666;
    font-size: 0.82rem;
    margin: 0 0 16px;
  }
  .btn {
    padding: 4px 14px;
    border: none;
    border-radius: 4px;
    background: #1565c0;
    color: #fff;
    cursor: pointer;
    font-size: 0.82rem;
  }
  .grid {
    position: relative;
    background: #0f0f1a;
    border: 1px solid #2a2a4a;
    border-radius: 6px;
    overflow: hidden;
  }
  .cell {
    position: absolute;
    border: 1px dashed #1e1e32;
    transition: background 0.1s;
  }
  .cell.highlight:hover {
    background: rgba(79, 195, 247, 0.08);
  }
  .screen-tile {
    position: absolute;
    background: #1a1a2e;
    border: 2px solid #3a3a5a;
    border-radius: 4px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    cursor: grab;
    user-select: none;
    transition: border-color 0.15s;
    z-index: 10;
  }
  .screen-tile:hover {
    border-color: #4fc3f7;
  }
  .screen-tile.local {
    border-color: #4caf50;
  }
  .screen-tile.is-dragging {
    opacity: 0.6;
    cursor: grabbing;
  }
  .screen-name {
    font-size: 0.7rem;
    font-weight: 600;
    color: #d0d0e8;
  }
  .screen-res {
    font-size: 0.6rem;
    color: #888;
  }
</style>
