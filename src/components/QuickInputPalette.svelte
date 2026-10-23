<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Search, Clock, Zap, Users, Coffee } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import { invoke } from '@tauri-apps/api/core';
  
  interface Props {
    visible?: boolean;
    onClose?: () => void;
  }
  
  let { visible = $bindable(false), onClose }: Props = $props();
  
  let searchQuery = $state('');
  let recentTasks = $state<string[]>([]);
  let selectedIndex = $state(0);
  
  const quickActions = [
    { id: 'work', icon: Clock, label: '開始工作', type: 'work' },
    { id: 'meeting', icon: Users, label: '會議', type: 'meeting' },
    { id: 'lunch', icon: Coffee, label: '午休', type: 'lunch' },
    { id: 'interrupt', icon: Zap, label: '插隊', type: 'interrupt' },
  ];
  
  $effect(() => {
    if (visible) {
      loadRecentTasks();
      searchQuery = '';
      selectedIndex = 0;
    }
  });
  
  async function loadRecentTasks() {
    try {
      const tasks = await invoke<Array<{task_name: string}>>('get_recent_tasks', { limit: 5 });
      recentTasks = tasks.map(t => t.task_name);
    } catch (err) {
      console.error('Failed to load recent tasks:', err);
    }
  }
  
  function handleKeydown(e: KeyboardEvent) {
    if (!visible) return;
    
    if (e.key === 'Escape') {
      close();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, recentTasks.length + quickActions.length - 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      selectItem(selectedIndex);
    }
  }
  
  function selectItem(index: number) {
    if (index < quickActions.length) {
      const action = quickActions[index];
      startTask(searchQuery || action.label, action.type);
    } else {
      const taskName = recentTasks[index - quickActions.length];
      startTask(taskName, 'work');
    }
  }
  
  async function startTask(taskName: string, slotType: string) {
    try {
      await invoke('start_task', {
        request: {
          task_name: taskName,
          slot_type: slotType
        }
      });
      close();
    } catch (err) {
      console.error('Failed to start task:', err);
    }
  }
  
  function close() {
    visible = false;
    if (onClose) onClose();
  }
  
  function handleGlobalEvent(e: Event) {
    visible = true;
  }
  
  onMount(() => {
    document.addEventListener('keydown', handleKeydown);
    window.addEventListener('show-quick-input', handleGlobalEvent);
  });
  
  onDestroy(() => {
    document.removeEventListener('keydown', handleKeydown);
    window.removeEventListener('show-quick-input', handleGlobalEvent);
  });
</script>

{#if visible}
  <div 
    class="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-[20vh]"
    onclick={close}
  >
    <div 
      class="w-[560px] rounded-lg border border-border bg-card shadow-lg"
      onclick={(e) => e.stopPropagation()}
    >
      <!-- Search Input -->
      <div class="flex items-center gap-3 border-b border-border px-4 py-3">
        <Search class="size-5 text-muted-foreground" />
        <input
          type="text"
          bind:value={searchQuery}
          placeholder={$t('quick_input.placeholder')}
          autofocus
          class="flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground"
        />
        <kbd class="rounded bg-muted px-2 py-1 text-xs font-mono text-muted-foreground">Esc</kbd>
      </div>
      
      <!-- Results -->
      <div class="max-h-[360px] overflow-y-auto p-2">
        {#if searchQuery}
          <!-- Custom task -->
          <button
            onclick={() => startTask(searchQuery, 'work')}
            class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-left hover:bg-accent"
            class:bg-accent={selectedIndex === 0}
          >
            <div class="flex size-8 items-center justify-center rounded bg-primary/10">
              <Clock class="size-4 text-primary" />
            </div>
            <div class="flex-1">
              <div class="text-sm font-medium">{searchQuery}</div>
              <div class="text-xs text-muted-foreground">開始新任務</div>
            </div>
          </button>
        {:else}
          <!-- Quick Actions -->
          <div class="mb-3">
            <div class="px-3 py-1 text-xs font-medium text-muted-foreground">{$t('quick_input.quick_blocks')}</div>
            {#each quickActions as action, i}
              <button
                onclick={() => selectItem(i)}
                class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-left hover:bg-accent"
                class:bg-accent={selectedIndex === i}
              >
                <div class="flex size-8 items-center justify-center rounded bg-muted">
                  <svelte:component this={action.icon} class="size-4" />
                </div>
                <span class="text-sm">{action.label}</span>
              </button>
            {/each}
          </div>
          
          <!-- Recent Tasks -->
          {#if recentTasks.length > 0}
            <div>
              <div class="px-3 py-1 text-xs font-medium text-muted-foreground">{$t('quick_input.recent_tasks')}</div>
              {#each recentTasks as task, i}
                <button
                  onclick={() => selectItem(quickActions.length + i)}
                  class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-left hover:bg-accent"
                  class:bg-accent={selectedIndex === quickActions.length + i}
                >
                  <div class="flex size-8 items-center justify-center rounded bg-muted">
                    <Clock class="size-4" />
                  </div>
                  <span class="text-sm truncate">{task}</span>
                </button>
              {/each}
            </div>
          {/if}
        {/if}
      </div>
    </div>
  </div>
{/if}
