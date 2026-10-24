<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  interface StateData {
    status: 'idle' | 'working' | 'interrupt' | 'paused';
    current_slot_id: number | null;
    paused_slot_id: number | null;
    remaining_seconds: number | null;
  }

  interface TimeSlot {
    id: number;
    task_name: string;
    slot_type: string;
    start_ts: string;
    end_ts: string | null;
  }

  let state = $state<StateData>({
    status: 'idle',
    current_slot_id: null,
    paused_slot_id: null,
    remaining_seconds: null,
  });

  let currentSlot = $state<TimeSlot | null>(null);
  let elapsedSeconds = $state(0);
  let menuOpen = $state(false);
  let hovered = $state(false);
  let pausedHold = false;

  const isIdle = $derived(state.status === 'idle');
  const isInterrupt = $derived(state.status === 'interrupt');
  const taskDisplay = $derived.by(() => {
    const name = currentSlot?.task_name?.trim();
    if (!name) return '未命名任務';
    return name.replace(/^插隊：/, '');
  });
  const menuItems = $derived.by(() => {
    const items = [
      { id: 'main', label: '開啟主視窗' },
      { id: 'quick', label: '快速輸入' },
    ];
    if (state.status === 'working') items.push({ id: 'pause', label: '暫停' });
    if (state.status === 'paused') items.push({ id: 'pause', label: '繼續' });
    if (state.status === 'working' || state.status === 'paused') {
      items.push({ id: 'end', label: '結束' });
    }
    if (state.status === 'interrupt') items.push({ id: 'end', label: '返回工作' });
    return items;
  });

  function secondsSince(ts: string): number {
    const parsed = Date.parse(ts);
    if (Number.isNaN(parsed)) return 0;
    return Math.max(0, Math.floor((Date.now() - parsed) / 1000));
  }

  async function loadState() {
    try {
      const next = await invoke<StateData>('get_app_state');
      state = next;
      if (next.current_slot_id) {
        currentSlot = await invoke<TimeSlot | null>('get_current_slot');
      } else {
        currentSlot = null;
      }

      if (next.status === 'interrupt' && currentSlot?.start_ts) {
        elapsedSeconds = secondsSince(String(currentSlot.start_ts));
        pausedHold = false;
        return;
      }

      const elapsed = await invoke<number | null>('get_elapsed_seconds');
      const value = typeof elapsed === 'number' && elapsed >= 0 ? elapsed : 0;
      if (next.status === 'paused') {
        if (!pausedHold) {
          elapsedSeconds = value;
          pausedHold = true;
        }
      } else {
        pausedHold = false;
        elapsedSeconds = next.status === 'idle' ? 0 : value;
      }
    } catch (err) {
      console.error('Failed to load state:', err);
    }
  }

  function formatTime(seconds: number): string {
    const safe = Math.max(0, Math.floor(seconds));
    const mins = Math.floor(safe / 60);
    const secs = safe % 60;
    return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  }

  async function togglePause() {
    try {
      if (state.status === 'paused') {
        await invoke('resume_task');
      } else if (state.status === 'working') {
        await invoke('pause_task');
      }
      await loadState();
    } catch (err) {
      console.error('Failed to toggle pause:', err);
    }
  }

  async function endTask() {
    try {
      if (state.status === 'interrupt') {
        await invoke('end_interrupt_and_resume');
      } else {
        await invoke('end_task', { note: null });
      }
      await loadState();
      window.dispatchEvent(new CustomEvent('task-started'));
    } catch (err) {
      console.error('Failed to end task:', err);
    }
  }

  async function focusMain() {
    try {
      await invoke('focus_main_window');
    } catch (err) {
      console.error('Failed to focus main window:', err);
    }
  }

  async function openQuickInput() {
    try {
      await invoke('open_quick_input');
    } catch (err) {
      console.error('Failed to open quick input:', err);
    }
  }

  async function syncWindowHeight() {
    const height = menuOpen ? 56 + menuItems.length * 32 : 48;
    try {
      await invoke('set_minibar_height', { height });
    } catch (err) {
      console.error('Failed to resize minibar:', err);
    }
  }

  function openMenu(event: MouseEvent) {
    event.preventDefault();
    menuOpen = true;
    syncWindowHeight();
  }

  function closeMenu() {
    if (!menuOpen) return;
    menuOpen = false;
    syncWindowHeight();
  }

  async function runMenu(id: string) {
    closeMenu();
    if (id === 'main') await focusMain();
    if (id === 'quick') await openQuickInput();
    if (id === 'pause') await togglePause();
    if (id === 'end') await endTask();
  }

  function onBarDoubleClick(event: MouseEvent) {
    const target = event.target as HTMLElement | null;
    if (target?.closest('button')) return;
    focusMain();
  }

  onMount(() => {
    loadState();
    syncWindowHeight();
    const timer = window.setInterval(loadState, 1000);
    return () => window.clearInterval(timer);
  });
</script>

<svelte:window onblur={closeMenu} onkeydown={(event) => event.key === 'Escape' && closeMenu()} />

<div class="shell {isIdle ? 'dashed' : ''} {isInterrupt ? 'dashed' : ''} {!hovered && !menuOpen ? 'dim' : ''}">
  <div
    class="bar"
    role="toolbar"
    tabindex="0"
    aria-label="迷你工具列"
    title="雙擊開啟主視窗 · 拖曳移動 · 右鍵選單"
    onmouseenter={() => (hovered = true)}
    onmouseleave={() => (hovered = false)}
    oncontextmenu={openMenu}
    ondblclick={onBarDoubleClick}
  >
    {#if isInterrupt}
      <span class="badge" data-tauri-drag-region aria-hidden="true">
        <svg viewBox="0 0 16 16" width="12" height="12">
          <path d="M9 1 3.5 9H8l-1 6 5.5-8H8l1-6Z" fill="currentColor" />
        </svg>
      </span>
    {:else if isIdle}
      <span class="hollow" data-tauri-drag-region aria-hidden="true"></span>
    {:else}
      <span class="dot" data-tauri-drag-region aria-hidden="true"></span>
    {/if}

    {#if isIdle}
      <span class="idle-copy" data-tauri-drag-region>未記錄 · 按</span>
      <button class="kbd" type="button" onclick={openQuickInput}>Ctrl</button>
      <button class="kbd" type="button" onclick={openQuickInput}>Alt</button>
      <button class="kbd" type="button" onclick={openQuickInput}>T</button>
    {:else}
      <span class="task" data-tauri-drag-region>{taskDisplay}</span>
      <span class="timer" data-tauri-drag-region>{formatTime(elapsedSeconds)}</span>
      <button
        class="icon-btn"
        type="button"
        onclick={isInterrupt ? endTask : togglePause}
        aria-label={isInterrupt ? '返回工作' : state.status === 'paused' ? '繼續' : '暫停'}
      >
        {#if isInterrupt}
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <path d="M6.5 3.5 3 7l3.5 3.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
            <path d="M3.5 7H10a3 3 0 0 1 0 6H8" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
          </svg>
        {:else if state.status === 'paused'}
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <path d="M5 3.5v9l8-4.5-8-4.5Z" fill="currentColor" />
          </svg>
        {:else}
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <rect x="3" y="2.5" width="3.2" height="11" rx="0.6" fill="currentColor" />
            <rect x="9.8" y="2.5" width="3.2" height="11" rx="0.6" fill="currentColor" />
          </svg>
        {/if}
      </button>
      {#if !isInterrupt}
        <button class="icon-btn" type="button" onclick={focusMain} aria-label="開啟主視窗">
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <path d="M6 3.5H3.5V12.5H12.5V10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
            <path d="M8.5 3.5H12.5V7.5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" />
            <path d="M12.2 3.8 7.2 8.8" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
          </svg>
        </button>
      {/if}
    {/if}
  </div>

  {#if menuOpen}
    <div class="menu" role="menu">
      {#each menuItems as item (item.id)}
        <button class="menu-item" type="button" role="menuitem" onclick={() => runMenu(item.id)}>
          {item.label}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .shell {
    width: 240px;
    color: hsl(var(--foreground));
  }

  .shell.dim {
    opacity: 0.4;
  }

  .bar {
    box-sizing: border-box;
    height: 48px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 10px;
    border: 1px solid hsl(var(--border));
    border-radius: 10px;
    background: hsl(var(--background) / 0.96);
    box-shadow: 0 8px 24px hsl(240 10% 3.9% / 0.12);
  }

  .dashed .bar {
    border-style: dashed;
  }

  .dot,
  .hollow {
    width: 8px;
    height: 8px;
    border-radius: 999px;
    flex-shrink: 0;
  }

  .dot {
    background: hsl(var(--foreground));
  }

  .hollow {
    border: 1.5px solid hsl(var(--foreground));
  }

  .badge {
    width: 22px;
    height: 22px;
    border-radius: 6px;
    border: 1px solid hsl(var(--border));
    display: grid;
    place-items: center;
    flex-shrink: 0;
  }

  .task,
  .idle-copy {
    min-width: 0;
    flex: 1;
    font-size: 13px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .idle-copy {
    flex: 0 1 auto;
  }

  .timer {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-size: 13px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }

  .kbd,
  .icon-btn,
  .menu-item {
    border: 0;
    color: inherit;
    background: transparent;
    cursor: pointer;
  }

  .kbd {
    height: 18px;
    padding: 0 5px;
    border-radius: 4px;
    background: hsl(var(--muted));
    color: hsl(var(--muted-foreground));
    font-size: 10px;
    font-weight: 600;
  }

  .icon-btn {
    width: 24px;
    height: 24px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    flex-shrink: 0;
  }

  .icon-btn:hover,
  .menu-item:hover {
    background: hsl(var(--accent));
  }

  .menu {
    margin-top: 6px;
    padding: 4px;
    border: 1px solid hsl(var(--border));
    border-radius: 10px;
    background: hsl(var(--popover));
    box-shadow: 0 8px 24px hsl(240 10% 3.9% / 0.16);
  }

  .menu-item {
    width: 100%;
    height: 32px;
    padding: 0 8px;
    border-radius: 6px;
    text-align: left;
    font-size: 13px;
  }
</style>
