<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Zap } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import { invoke } from '@tauri-apps/api/core';
  
  interface Props {
    visible?: boolean;
    onClose?: () => void;
  }
  
  let { visible = $bindable(false), onClose }: Props = $props();
  
  let who = $state('');
  let reason = $state('');
  let recentPeople = $state<string[]>([]);
  
  $effect(() => {
    if (visible) {
      loadRecentPeople();
      who = '';
      reason = '';
    }
  });
  
  async function loadRecentPeople() {
    try {
      const people = await invoke<Array<{person_name: string}>>('get_recent_people', { limit: 5 });
      recentPeople = people.map(p => p.person_name);
    } catch (err) {
      console.error('Failed to load recent people:', err);
    }
  }
  
  async function startInterrupt() {
    if (!who.trim()) return;
    
    try {
      await invoke('start_interrupt', {
        who: who.trim(),
        reason: reason.trim() || null
      });
      close();
    } catch (err) {
      console.error('Failed to start interrupt:', err);
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
    window.addEventListener('show-interrupt', handleGlobalEvent);
  });
  
  onDestroy(() => {
    window.removeEventListener('show-interrupt', handleGlobalEvent);
  });
</script>

{#if visible}
  <div 
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    onclick={close}
  >
    <div 
      class="w-[480px] rounded-lg border border-border bg-card p-6 shadow-lg"
      onclick={(e) => e.stopPropagation()}
    >
      <!-- Header -->
      <div class="mb-4 flex items-center gap-3">
        <div class="flex size-10 items-center justify-center rounded-md bg-accent">
          <Zap class="size-5 text-accent-foreground" />
        </div>
        <div>
          <h2 class="text-lg font-semibold">{$t('interrupt.title')}</h2>
          <div class="text-sm text-muted-foreground">{$t('interrupt.current_task_paused')}</div>
        </div>
      </div>
      
      <!-- Form -->
      <div class="space-y-4">
        <!-- Who -->
        <div class="space-y-2">
          <label class="text-sm font-medium">{$t('interrupt.who')}</label>
          <input
            type="text"
            bind:value={who}
            placeholder={$t('interrupt.who_placeholder')}
            autofocus
            class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm"
          />
          
          <!-- Recent People Chips -->
          {#if recentPeople.length > 0}
            <div class="flex flex-wrap gap-2">
              {#each recentPeople as person}
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
        
        <!-- Reason -->
        <div class="space-y-2">
          <label class="text-sm font-medium">{$t('interrupt.reason')}</label>
          <input
            type="text"
            bind:value={reason}
            placeholder={$t('interrupt.reason_placeholder')}
            class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm"
          />
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
