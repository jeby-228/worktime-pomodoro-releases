<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Play, Pause, Square, Zap } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import { invoke } from '@tauri-apps/api/core';
  
  interface Props {
    compact?: boolean;
  }
  
  let { compact = false }: Props = $props();
  
  interface StateData {
    status: 'idle' | 'working' | 'paused' | 'interrupt';
    current_task: string | null;
    elapsed_seconds: number;
    remaining_seconds: number;
  }
  
  let state = $state<StateData>({
    status: 'idle',
    current_task: null,
    elapsed_seconds: 0,
    remaining_seconds: 0,
  });
  
  let pomodoroLength = $state(25 * 60);
  let updateInterval: number | undefined;
  
  let displayTime = $derived(() => {
    const seconds = state.status === 'idle' ? 0 : (state.elapsed_seconds || 0);
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
  });
  
  let progress = $derived(() => {
    if (state.status === 'idle') return 0;
    const elapsed = state.elapsed_seconds || 0;
    return Math.min(100, (elapsed / pomodoroLength) * 100);
  });
  
  onMount(() => {
    loadState();
    loadSettings();
    
    updateInterval = setInterval(() => {
      if (state.status === 'working' || state.status === 'interrupt') {
        loadState();
      }
    }, 1000) as unknown as number;
  });
  
  onDestroy(() => {
    if (updateInterval) clearInterval(updateInterval);
  });
  
  async function loadState() {
    try {
      const data = await invoke<StateData>('get_app_state');
      state = data;
      
      const elapsed = await invoke<number | null>('get_elapsed_seconds');
      const remaining = await invoke<number | null>('get_remaining_seconds');
      
      if (elapsed !== null && !isNaN(elapsed)) {
        state.elapsed_seconds = elapsed;
      } else {
        state.elapsed_seconds = 0;
      }
      
      if (remaining !== null && !isNaN(remaining)) {
        state.remaining_seconds = remaining;
      } else {
        state.remaining_seconds = 0;
      }
    } catch (err) {
      console.error('Failed to load state:', err);
      // Reset to safe idle state on error
      state = {
        status: 'idle',
        current_task: null,
        elapsed_seconds: 0,
        remaining_seconds: 0,
      };
    }
  }
  
  async function loadSettings() {
    try {
      const settings = await invoke<any>('get_settings');
      if (settings.pomodoro_length_minutes) {
        pomodoroLength = settings.pomodoro_length_minutes * 60;
      }
    } catch (err) {
      console.error('Failed to load settings:', err);
    }
  }
  
  async function startTask() {
    // TODO: show quick input dialog
    // For now, start with a default task
    try {
      await invoke('start_task', {
        request: {
          task_name: '新任務',
          slot_type: 'work'
        }
      });
      await loadState();
    } catch (err) {
      console.error('Failed to start task:', err);
    }
  }
  
  async function pauseTask() {
    try {
      await invoke('pause_task');
      await loadState();
    } catch (err) {
      console.error('Failed to pause task:', err);
    }
  }
  
  async function resumeTask() {
    try {
      await invoke('resume_task');
      await loadState();
    } catch (err) {
      console.error('Failed to resume task:', err);
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
  
  async function startInterrupt() {
    // TODO: show interrupt dialog
    // For now, start a default interrupt
    try {
      await invoke('start_interrupt', {
        who: '臨時事件',
        reason: null
      });
      await loadState();
    } catch (err) {
      console.error('Failed to start interrupt:', err);
    }
  }
</script>

<div class="flex flex-col {compact ? 'h-full' : ''} rounded-lg border border-border bg-card p-6 text-card-foreground shadow-sm">
  <!-- Header -->
  <div class="mb-4 flex items-center justify-between">
    <div class="text-sm text-muted-foreground">
      {#if state.status === 'idle'}
        {$t('timer.idle')}
      {:else if state.status === 'working'}
        {$t('timer.working')}
      {:else if state.status === 'paused'}
        {$t('timer.paused')}
      {:else}
        {$t('timer.interrupt')}
      {/if}
    </div>
    {#if state.current_task}
      <div class="text-sm font-medium">
        {state.current_task}
      </div>
    {/if}
  </div>
  
  <!-- Big Timer Display -->
  <div class="mb-6 text-center">
    <div class="text-7xl font-bold tracking-tight tabular-nums">
      {displayTime()}
    </div>
    {#if state.status !== 'idle'}
      <div class="mt-2 h-2 overflow-hidden rounded-full bg-muted">
        <div 
          class="h-full bg-accent transition-all duration-300"
          style="width: {progress()}%"
        ></div>
      </div>
    {/if}
  </div>
  
  <!-- Controls -->
  <div class="flex gap-3">
    {#if state.status === 'idle'}
      <button 
        onclick={startTask}
        class="flex flex-1 items-center justify-center gap-2 rounded-md bg-primary px-4 py-2.5 text-sm font-medium text-primary-foreground hover:bg-primary/90"
      >
        <Play class="size-4" />
        {$t('timer.start_task')}
      </button>
    {:else if state.status === 'paused'}
      <button 
        onclick={resumeTask}
        class="flex flex-1 items-center justify-center gap-2 rounded-md bg-primary px-4 py-2.5 text-sm font-medium text-primary-foreground hover:bg-primary/90"
      >
        <Play class="size-4" />
        {$t('timer.resume')}
      </button>
      <button 
        onclick={endTask}
        class="flex items-center justify-center gap-2 rounded-md border border-border px-4 py-2.5 text-sm font-medium hover:bg-accent hover:text-accent-foreground"
      >
        <Square class="size-4" />
        {$t('timer.end_task')}
      </button>
    {:else}
      <button 
        onclick={pauseTask}
        class="flex items-center justify-center gap-2 rounded-md border border-border px-4 py-2.5 text-sm font-medium hover:bg-accent hover:text-accent-foreground"
      >
        <Pause class="size-4" />
        {$t('timer.pause')}
      </button>
      <button 
        onclick={endTask}
        class="flex items-center justify-center gap-2 rounded-md border border-border px-4 py-2.5 text-sm font-medium hover:bg-accent hover:text-accent-foreground"
      >
        <Square class="size-4" />
        {$t('timer.end_task')}
      </button>
      <button 
        onclick={startInterrupt}
        class="flex items-center justify-center gap-2 rounded-md bg-accent px-4 py-2.5 text-sm font-medium text-accent-foreground hover:bg-accent/80"
        title={$t('timer.interrupt_hint')}
      >
        <Zap class="size-4" />
        {$t('timer.interrupt_btn')}
      </button>
    {/if}
  </div>
  
  {#if !compact}
    <div class="mt-4 text-center text-xs text-muted-foreground">
      {$t('timer.quick_input_hint')} · {$t('timer.interrupt_hint')}
    </div>
  {/if}
</div>
