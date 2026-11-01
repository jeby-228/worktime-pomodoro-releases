<script lang="ts">
  import { onMount } from 'svelte';
  import { Zap } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import { invoke } from '@tauri-apps/api/core';
  import CategorySelect from './CategorySelect.svelte';

  interface Props {
    visible?: boolean;
    onClose?: () => void;
  }
  
  let { visible = $bindable(false), onClose }: Props = $props();
  
  let who = $state('');
  let reason = $state('');
  let recentPeople = $state<string[]>([]);
  let submitError = $state('');
  let categoryId = $state<number | null>(null);

  function openDialog() {
    who = '';
    reason = '';
    submitError = '';
    categoryId = null;
    visible = true;
    loadRecentPeople();
    loadDefaultCategory();
  }

  async function loadDefaultCategory() {
    try {
      categoryId = await invoke<number | null>('get_default_category', { slotType: 'interrupt' });
    } catch (err) {
      console.error('Failed to load default category:', err);
    }
  }

  function handleGlobalEvent() {
    openDialog();
  }

  async function loadRecentPeople() {
    try {
      const people = await invoke<Array<{name: string}>>('get_recent_people', { limit: 5 });
      recentPeople = people.map(p => p.name);
    } catch (err) {
      console.error('Failed to load recent people:', err);
    }
  }
  
  async function startInterrupt() {
    if (!who.trim()) return;
    
    try {
      await invoke('start_interrupt', {
        request: {
          who: who.trim(),
          reason: reason.trim() || null,
          category_id: categoryId,
        },
      });
      window.dispatchEvent(new CustomEvent('task-started'));
      close();
    } catch (err) {
      submitError = String(err);
    }
  }
  
  function close() {
    visible = false;
    if (onClose) onClose();
  }

  onMount(() => {
    window.addEventListener('show-interrupt', handleGlobalEvent);
    return () => window.removeEventListener('show-interrupt', handleGlobalEvent);
  });
</script>

{#if visible}
  <div class="fixed inset-0 z-50 flex items-center justify-center">
    <button
      type="button"
      class="absolute inset-0 bg-black/50"
      aria-label={$t('common.cancel')}
      onclick={close}
    ></button>
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="interrupt-title"
      class="relative w-[480px] rounded-lg border border-border bg-card p-6 shadow-lg"
    >
      <!-- Header -->
      <div class="mb-4 flex items-center gap-3">
        <div class="flex size-10 items-center justify-center rounded-md bg-accent">
          <Zap class="size-5 text-accent-foreground" />
        </div>
        <div>
          <h2 id="interrupt-title" class="text-lg font-semibold">{$t('interrupt.title')}</h2>
          <div class="text-sm text-muted-foreground">{$t('interrupt.current_task_paused')}</div>
        </div>
      </div>
      
      <!-- Form -->
      <div class="space-y-4">
        <!-- Who -->
        <div class="space-y-2">
          <label for="interrupt-who" class="text-sm font-medium">{$t('interrupt.who')}</label>
          <input
            id="interrupt-who"
            type="text"
            bind:value={who}
            placeholder={$t('interrupt.who_placeholder')}
            class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm"
          />
          
          <!-- Recent People Chips -->
          {#if recentPeople.length > 0}
            <div class="flex flex-wrap gap-2">
              {#each recentPeople as person (person)}
                <button
                  onclick={() => who = person}
                  class="rounded-full bg-muted px-3 py-1 text-xs hover:bg-muted/80"
                >
                  {person}
                </button>
              {/each}
            </div>
          {/if}
        </div>
        
        <!-- Category -->
        <div class="space-y-2">
          <label for="interrupt-category" class="text-sm font-medium">{$t('categories.label')}</label>
          <CategorySelect id="interrupt-category" bind:value={categoryId} />
        </div>

        <!-- Reason -->
        <div class="space-y-2">
          <label for="interrupt-reason" class="text-sm font-medium">{$t('interrupt.reason')}</label>
          <input
            id="interrupt-reason"
            type="text"
            bind:value={reason}
            placeholder={$t('interrupt.reason_placeholder')}
            class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm"
          />
          {#if submitError}
            <p class="text-sm text-destructive">{submitError}</p>
          {/if}
        </div>
      </div>
      
      <!-- Actions -->
      <div class="mt-6 flex justify-end gap-3">
        <button
          onclick={close}
          class="rounded-md border border-border px-4 py-2 text-sm hover:bg-accent"
        >
          {$t('common.cancel')}
        </button>
        <button
          onclick={startInterrupt}
          disabled={!who.trim()}
          class="rounded-md bg-accent px-4 py-2 text-sm font-medium text-accent-foreground hover:bg-accent/80 disabled:opacity-50"
        >
          開始插隊
        </button>
      </div>
    </div>
  </div>
{/if}
