<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Clock, Play, Pause, Square } from 'lucide-svelte';

  interface AppState {
    status: 'idle' | 'working' | 'paused' | 'interrupt';
    current_slot_id: number | null;
    task_title: string | null;
    elapsed_seconds: number;
    remaining_seconds: number;
  }

  let state = $state<AppState>({
    status: 'idle',
    current_slot_id: null,
    task_title: null,
    elapsed_seconds: 0,
    remaining_seconds: 0,
  });

  let interval: ReturnType<typeof setInterval> | null = null;

  async function loadState() {
    try {
      state = await invoke('get_app_state');
    } catch (err) {
      console.error('Failed to load state:', err);
    }
  }

  function formatTime(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, '0')}`;
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
      await invoke('end_task', { note: null });
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

  const statusColor = $derived(() => {
    switch (state.status) {
      case 'working': return 'bg-green-500';
      case 'paused': return 'bg-yellow-500';
      case 'interrupt': return 'bg-blue-500';
      default: return 'bg-zinc-500';
    }
  });
</script>

<div class="flex h-12 items-center gap-2 bg-background/95 backdrop-blur px-3 border-b border-border">
  <!-- Status indicator -->
  <div class="size-2 rounded-full {statusColor()}" />

  <!-- Task info -->
  <div class="flex-1 min-w-0 text-sm">
    {#if state.task_title}
      <div class="font-medium truncate">{state.task_title}</div>
    {:else}
      <div class="text-muted-foreground">無任務</div>
    {/if}
  </div>

  <!-- Time display -->
  {#if state.status === 'working' || state.status === 'paused'}
    <div class="text-sm font-mono tabular-nums">
      {formatTime(state.elapsed_seconds)}
    </div>
  {/if}

  <!-- Controls -->
  <div class="flex gap-1">
    {#if state.status === 'working' || state.status === 'paused'}
      <button
        onclick={togglePause}
        class="p-1 hover:bg-accent rounded"
        title={state.status === 'paused' ? '繼續' : '暫停'}
      >
        {#if state.status === 'paused'}
          <Play size={14} />
        {:else}
          <Pause size={14} />
        {/if}
      </button>
      <button
        onclick={endTask}
        class="p-1 hover:bg-accent rounded text-destructive"
        title="結束"
      >
        <Square size={14} />
      </button>
    {/if}
  </div>
</div>
