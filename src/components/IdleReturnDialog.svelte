<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { Clock, Coffee, Users, XCircle } from 'lucide-svelte';
  import { t } from '$lib/i18n';

  let { open = $bindable(false), idleMinutes = $bindable(0), onResolved = () => {} } = $props<{
    open: boolean;
    idleMinutes: number;
    onResolved?: () => void;
  }>();

  let processing = $state(false);
  let error = $state('');

  type IdleAction = 'continue' | 'lunch' | 'meeting' | 'discard';

  async function handleAction(action: IdleAction) {
    processing = true;
    error = '';

    try {
      await invoke('handle_idle_return', { action, idleMinutes });
      onResolved();
      open = false;
    } catch (err: any) {
      error = err.toString();
    } finally {
      processing = false;
    }
  }

  const actions = [
    {
      id: 'continue' as const,
      icon: Clock,
      label: $derived($t('idle_return.continue_task')),
      description: $derived($t('idle_return.continue_desc')),
      color: 'text-primary',
      bgHover: 'hover:bg-primary/10',
    },
    {
      id: 'lunch' as const,
      icon: Coffee,
      label: $derived($t('idle_return.lunch')),
      description: $derived($t('idle_return.lunch_desc')),
      color: 'text-blue-600 dark:text-blue-400',
      bgHover: 'hover:bg-blue-500/10',
    },
    {
      id: 'meeting' as const,
      icon: Users,
      label: $derived($t('idle_return.meeting')),
      description: $derived($t('idle_return.meeting_desc')),
      color: 'text-green-600 dark:text-green-400',
      bgHover: 'hover:bg-green-500/10',
    },
    {
      id: 'discard' as const,
      icon: XCircle,
      label: $derived($t('idle_return.discard')),
      description: $derived($t('idle_return.discard_desc')),
      color: 'text-muted-foreground',
      bgHover: 'hover:bg-muted',
    },
  ];
</script>

{#if open}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
    <!-- Backdrop -->
    <div class="absolute inset-0 bg-black/50" />

    <!-- Dialog -->
    <div class="relative w-full max-w-lg bg-background rounded-lg shadow-xl">
      <!-- Header -->
      <div class="px-6 py-4 border-b border-border">
        <h2 class="text-lg font-semibold text-foreground">
          {$t('idle_return.title')}
        </h2>
        <p class="mt-1 text-sm text-muted-foreground">
          {$t('idle_return.message', { minutes: idleMinutes })}
        </p>
      </div>

      <!-- Content -->
      <div class="px-6 py-4 space-y-2">
        {#each actions as action}
          <button
            onclick={() => handleAction(action.id)}
            disabled={processing}
            class="w-full flex items-start gap-3 p-3 rounded-md border border-border {action.bgHover} transition-colors disabled:opacity-50 text-left"
          >
            <svelte:component this={action.icon} size={20} class="{action.color} mt-0.5" />
            <div class="flex-1">
              <div class="font-medium text-foreground">{action.label}</div>
              <div class="text-sm text-muted-foreground">{action.description}</div>
            </div>
          </button>
        {/each}

        {#if error}
          <div class="mt-4 px-3 py-2 bg-destructive/10 border border-destructive/20 rounded-md text-sm text-destructive">
            {error}
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}
