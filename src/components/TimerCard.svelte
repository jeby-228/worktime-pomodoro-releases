<script lang="ts">
  import { onMount } from 'svelte';
  import { Play, Pause, Square, Zap } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import { invoke } from '@tauri-apps/api/core';
  import CategorySelect from './CategorySelect.svelte';

  interface Props {
    compact?: boolean;
  }
  
  let { compact = false }: Props = $props();
  
  interface StateData {
    status: 'idle' | 'working' | 'paused' | 'interrupt';
    current_task: string | null;
    slot_type: SlotType | null;
    category_name: string | null;
    resume_target: string | null;
    elapsed_seconds: number;
  }

  type SlotType = 'work' | 'meeting' | 'interrupt' | 'lunch';

  const slotTypes: SlotType[] = ['work', 'meeting', 'interrupt', 'lunch'];

  let state = $state<StateData>({
    status: 'idle',
    current_task: null,
    slot_type: null,
    category_name: null,
    resume_target: null,
    elapsed_seconds: 0,
  });

  let taskName = $state('');
  let slotType = $state<SlotType>('work');
  let categoryId = $state<number | null>(null);
  let startError = $state('');
  let actionError = $state('');
  let updateInterval: number | undefined;

  function secondsSince(ts: string): number {
    const trimmed = ts.replace(/(\.\d{3})\d+/, '$1');
    const parsed = Date.parse(trimmed);
    if (Number.isNaN(parsed)) return 0;
    return Math.max(0, Math.floor((Date.now() - parsed) / 1000));
  }
  
  let displayTime = $derived.by(() => {
    const seconds = state.status === 'idle' ? 0 : (state.elapsed_seconds || 0);
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
  });
  
  const statusLabel = $derived.by(() => {
    if (state.status === 'paused') return $t('timer.paused');
    if (state.status === 'interrupt') return $t('timer.interrupt');
    return state.slot_type ? $t(`slot_types.${state.slot_type}`) : $t('timer.working');
  });

  onMount(() => {
    loadState();
    loadDefaultCategory();
    window.addEventListener('task-started', loadState);
    
    updateInterval = setInterval(() => {
      if (state.status === 'working' || state.status === 'interrupt') {
        loadState();
      }
    }, 1000) as unknown as number;

    return () => {
      if (updateInterval) clearInterval(updateInterval);
      window.removeEventListener('task-started', loadState);
    };
  });
  
  async function loadDefaultCategory() {
    try {
      categoryId = await invoke<number | null>('get_default_category', { slotType });
    } catch (err) {
      console.error('Failed to load default category:', err);
    }
  }

  function chooseSlotType(next: SlotType) {
    slotType = next;
    startError = '';
    loadDefaultCategory();
  }

  async function loadState() {
    try {
      const [data, elapsed, slot] = await Promise.all([
        invoke<StateData>('get_app_state'),
        invoke<number | null>('get_elapsed_seconds'),
        invoke<{
          task_name?: string;
          start_ts?: string;
          slot_type?: SlotType;
          category_name?: string | null;
        } | null>('get_current_slot'),
      ]);
      const resume =
        data.status === 'interrupt'
          ? await invoke<{ task_name?: string; category_name?: string | null } | null>(
              'get_resume_slot',
            )
          : null;
      const finished = state.status !== 'idle' && data.status === 'idle';

      let nextElapsed =
        data.status === 'idle'
          ? 0
          : typeof elapsed === 'number' && !Number.isNaN(elapsed)
            ? elapsed
            : state.elapsed_seconds;

      if (data.status === 'interrupt' && slot?.start_ts) {
        const fromSlot = secondsSince(String(slot.start_ts));
        nextElapsed = fromSlot > 0 ? fromSlot : nextElapsed;
      } else if (data.status === 'working' && nextElapsed === 0 && slot?.start_ts) {
        const fromSlot = secondsSince(String(slot.start_ts));
        if (fromSlot > 1) nextElapsed = fromSlot;
      }

      state = {
        status: data.status,
        current_task: data.status === 'idle' ? null : slot?.task_name ?? null,
        slot_type: data.status === 'idle' ? null : slot?.slot_type ?? null,
        category_name: data.status === 'idle' ? null : slot?.category_name ?? null,
        resume_target: resume?.category_name || resume?.task_name || null,
        elapsed_seconds: nextElapsed,
      };
      if (finished) loadDefaultCategory();
    } catch (err) {
      console.error('Failed to load state:', err);
      // Reset to safe idle state on error
      state = {
        status: 'idle',
        current_task: null,
        slot_type: null,
        category_name: null,
        resume_target: null,
        elapsed_seconds: 0,
      };
    }
  }
  
  
  async function startTask() {
    // Only work needs a name; the other statuses can stand on their own.
    const name = taskName.trim() || (slotType === 'work' ? '' : $t(`slot_types.${slotType}`));
    if (!name) {
      startError = $t('errors.task_title_required');
      return;
    }

    startError = '';
    try {
      await invoke('start_task', {
        request: {
          task_name: name,
          slot_type: slotType,
          category_id: slotType === 'lunch' ? null : categoryId,
        }
      });
      taskName = '';
      await loadState();
      window.dispatchEvent(new CustomEvent('task-started'));
    } catch (err) {
      startError = String(err);
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
      window.dispatchEvent(new CustomEvent('task-started'));
    } catch (err) {
      console.error('Failed to end task:', err);
    }
  }
  
  function startInterrupt() {
    window.dispatchEvent(new CustomEvent('show-interrupt'));
  }

  async function endInterrupt() {
    actionError = '';
    try {
      await invoke('end_interrupt_and_resume');
      await loadState();
      window.dispatchEvent(new CustomEvent('task-started'));
    } catch (err) {
      actionError = String(err);
    }
  }
</script>

<div class="flex flex-col {compact ? 'h-full' : ''} rounded-lg border border-border bg-card p-6 text-card-foreground shadow-sm">
  <!-- Header -->
  <div class="mb-4 flex items-center justify-between">
    <div class="text-sm text-muted-foreground">
      {#if state.status === 'idle'}
        {$t('timer.idle')}
      {:else}
        {statusLabel}{#if state.slot_type && state.slot_type !== 'lunch'}{' · '}{state.category_name ?? $t('categories.uncategorized')}{/if}
      {/if}
    </div>
    {#if state.current_task}
      <div class="text-sm font-medium">
        {state.current_task}
      </div>
    {/if}
  </div>
  
  {#if state.status === 'idle'}
    <div class="mb-4">
      <div class="mb-1.5 text-sm font-medium">{$t('categories.status')}</div>
      <div class="grid grid-cols-4 gap-1 rounded-md border border-border p-1" role="group" aria-label={$t('categories.status')}>
        {#each slotTypes as type (type)}
          <button
            type="button"
            onclick={() => chooseSlotType(type)}
            aria-pressed={slotType === type}
            class="whitespace-nowrap rounded px-2 py-1.5 text-sm font-medium transition-colors {slotType === type
              ? 'bg-primary text-primary-foreground'
              : 'text-muted-foreground hover:bg-accent'}"
          >
            {$t(`slot_types.${type}`)}
          </button>
        {/each}
      </div>
    </div>

    {#if slotType !== 'lunch'}
      <div class="mb-4">
        <label for="current-category" class="mb-1.5 block text-sm font-medium">
          {$t('categories.label')}
        </label>
        <CategorySelect id="current-category" bind:value={categoryId} />
      </div>
    {/if}

    <div class="mb-6">
      <label for="current-task-name" class="mb-1.5 block text-sm font-medium">
        {$t('timer.task_name')}
      </label>
      <input
        id="current-task-name"
        type="text"
        bind:value={taskName}
        placeholder={$t('placeholders.task_title')}
        onkeydown={(event) => {
          if (event.key === 'Enter' && !event.isComposing) startTask();
        }}
        class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
      />
      {#if startError}
        <p class="mt-1.5 text-sm text-destructive">{startError}</p>
      {/if}
    </div>
  {/if}

  <!-- Big Timer Display -->
  <div class="mb-6 text-center">
    <div class="text-7xl font-bold tracking-tight tabular-nums">
      {displayTime}
    </div>
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
      {#if state.status !== 'interrupt'}
        <button 
          onclick={pauseTask}
          class="flex items-center justify-center gap-2 rounded-md border border-border px-4 py-2.5 text-sm font-medium hover:bg-accent hover:text-accent-foreground"
        >
          <Pause class="size-4" />
          {$t('timer.pause')}
        </button>
      {/if}
      {#if state.status === 'interrupt'}
        <button
          onclick={endInterrupt}
          class="flex flex-1 items-center justify-center gap-2 rounded-md bg-accent px-4 py-2.5 text-sm font-medium text-accent-foreground hover:bg-accent/80"
        >
          <Zap class="size-4" />
          {state.resume_target
            ? $t('categories.back_to', { target: state.resume_target })
            : $t('timer.back_to_work')}
        </button>
      {:else}
        <button
          onclick={startInterrupt}
          class="flex items-center justify-center gap-2 rounded-md bg-accent px-4 py-2.5 text-sm font-medium text-accent-foreground hover:bg-accent/80"
          title={$t('timer.interrupt_hint')}
        >
          <Zap class="size-4" />
          {$t('timer.interrupt_btn')}
        </button>
      {/if}
      <button
        onclick={endTask}
        class="flex items-center justify-center gap-2 rounded-md border border-border px-4 py-2.5 text-sm font-medium hover:bg-accent hover:text-accent-foreground"
      >
        <Square class="size-4" />
        {$t('timer.end_task')}
      </button>
    {/if}
  </div>

  {#if actionError}
    <p class="mt-3 text-sm text-destructive">{actionError}</p>
  {/if}
  
  {#if !compact}
    <div class="mt-4 text-center text-xs text-muted-foreground">
      {$t('timer.quick_input_hint')} · {$t('timer.interrupt_hint')}
    </div>
  {/if}
</div>
