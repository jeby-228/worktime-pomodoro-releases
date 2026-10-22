<script lang="ts">
  import { onMount } from 'svelte';
  import { Play, Pause, Square, Zap } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  
  interface Props {
    compact?: boolean;
  }
  
  let { compact = false }: Props = $props();
  
  let state = $state({
    status: 'idle' as 'idle' | 'working' | 'paused' | 'interrupt',
    currentTask: null as string | null,
    elapsedSeconds: 0,
    remainingSeconds: 0,
    pomodoroLength: 25 * 60,
  });
  
  let displayTime = $derived(() => {
    const seconds = state.status === 'idle' ? 0 : state.elapsedSeconds;
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
  });
  
  let progress = $derived(() => {
    if (state.status === 'idle') return 0;
    return Math.min(100, (state.elapsedSeconds / state.pomodoroLength) * 100);
  });
  
  onMount(() => {
    // TODO: load state from backend
    loadState();
    
    const interval = setInterval(() => {
      if (state.status === 'working' || state.status === 'interrupt') {
        state.elapsedSeconds++;
        if (state.remainingSeconds > 0) {
          state.remainingSeconds--;
        }
      }
    }, 1000);
    
    return () => clearInterval(interval);
  });
  
  async function loadState() {
    // TODO: invoke backend command
    console.log('Loading state...');
  }
  
  async function startTask() {
    // TODO: show quick input dialog
    console.log('Starting task');
  }
  
  async function pauseTask() {
    state.status = 'paused';
  }
  
  async function resumeTask() {
    state.status = 'working';
  }
  
  async function endTask() {
    state.status = 'idle';
    state.currentTask = null;
    state.elapsedSeconds = 0;
  }
  
  async function startInterrupt() {
    // TODO: show interrupt dialog
    console.log('Starting interrupt');
  }
</script>

<div class="flex flex-col {compact ? 'h-full' : ''} rounded-lg border border-border bg-card p-6 text-card-foreground shadow-sm">
  <!-- Header -->
  <div class="mb-4 flex items-center justify-between">
    <div class="text-sm text-muted-foreground">
      {#if state.status === 'idle'}
        {$t('timer.idle')}
      {:else if state.status === 'working'}
        {$t('timer.working')} · {$t('common.loading')} 3/8
      {:else if state.status === 'paused'}
        {$t('timer.paused')}
      {:else}
        {$t('timer.interrupt')}
      {/if}
    </div>
    {#if state.currentTask}
      <div class="text-sm font-medium">
        {state.currentTask}
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
