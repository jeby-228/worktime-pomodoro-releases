<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
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
  let interval: ReturnType<typeof setInterval> | null = null;

  async function loadState() {
    try {
      const newState: StateData = await invoke('get_app_state');
      state = newState;

      if (state.current_slot_id) {
        currentSlot = await invoke('get_current_slot');
        const elapsed: number = await invoke('get_elapsed_seconds');
        elapsedSeconds = elapsed || 0;
      } else {
        currentSlot = null;
        elapsedSeconds = 0;
      }
    } catch (err) {
      console.error('Failed to load state:', err);
    }
  }

  function formatTime(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
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
    } catch (err) {
      console.error('Failed to end task:', err);
    }
  }

  onMount(() => {
    loadState();
    interval = setInterval(loadState, 1000);
  });

  onDestroy(() => {
    if (interval) clearInterval(interval);
  });

  const statusDotClass = $derived(() => {
    switch (state.status) {
      case 'working': return 'status-working';
      case 'paused': return 'status-paused';
      case 'interrupt': return 'status-interrupt';
      default: return 'status-idle';
    }
  });

  const taskDisplay = $derived(() => {
    if (!currentSlot) return '無任務';
    return currentSlot.task_name || '未命名任務';
  });

  const showControls = $derived(
    state.status === 'working' || 
    state.status === 'paused' || 
    state.status === 'interrupt'
  );
</script>

<div class="minibar">
  <div class="status-dot {statusDotClass}"></div>
  
  <div class="task-name">{taskDisplay}</div>
  
  {#if showControls}
    <div class="timer">{formatTime(elapsedSeconds)}</div>
    
    <div class="controls">
      {#if state.status !== 'interrupt'}
        <button
          class="control-btn"
          onclick={togglePause}
          title={state.status === 'paused' ? '繼續' : '暫停'}
        >
          {state.status === 'paused' ? '▶' : '⏸'}
        </button>
      {/if}
      
      <button
        class="control-btn stop"
        onclick={endTask}
        title={state.status === 'interrupt' ? '結束插隊' : '結束'}
      >
        ⏹
      </button>
    </div>
  {/if}
  
  {#if state.status === 'interrupt'}
    <div class="interrupt-label">插隊中</div>
  {/if}
</div>

<style>
  .minibar {
    width: 240px;
    height: 48px;
    background: rgba(255, 255, 255, 0.95);
    backdrop-filter: blur(8px);
    border-radius: 8px;
    border: 1px solid rgba(0, 0, 0, 0.1);
    display: flex;
    align-items: center;
    padding: 0 12px;
    gap: 8px;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
  }

  @media (prefers-color-scheme: dark) {
    .minibar {
      background: rgba(24, 24, 27, 0.95);
      border-color: rgba(255, 255, 255, 0.1);
      box-shadow: 0 2px 8px rgba(0, 0, 0, 0.5);
    }
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .status-working {
    background: #22c55e;
  }

  .status-paused {
    background: #eab308;
  }

  .status-interrupt {
    background: #3b82f6;
  }

  .status-idle {
    background: #71717a;
  }

  .task-name {
    flex: 1;
    font-size: 13px;
    font-weight: 500;
    color: #18181b;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  @media (prefers-color-scheme: dark) {
    .task-name {
      color: #fafafa;
    }
  }

  .timer {
    font-family: 'JetBrains Mono', 'SF Mono', monospace;
    font-size: 13px;
    font-weight: 600;
    color: #18181b;
    tabular-nums: tabular-nums;
    flex-shrink: 0;
  }

  @media (prefers-color-scheme: dark) {
    .timer {
      color: #fafafa;
    }
  }

  .controls {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
  }

  .control-btn {
    width: 24px;
    height: 24px;
    border: none;
    background: rgba(0, 0, 0, 0.05);
    border-radius: 4px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    transition: background 0.15s;
  }

  .control-btn:hover {
    background: rgba(0, 0, 0, 0.1);
  }

  .control-btn.stop {
    color: #dc2626;
  }

  @media (prefers-color-scheme: dark) {
    .control-btn {
      background: rgba(255, 255, 255, 0.1);
    }

    .control-btn:hover {
      background: rgba(255, 255, 255, 0.15);
    }
  }

  .interrupt-label {
    font-size: 10px;
    font-weight: 600;
    color: #3b82f6;
    background: rgba(59, 130, 246, 0.1);
    padding: 2px 6px;
    border-radius: 4px;
    flex-shrink: 0;
  }
</style>
