<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { X, Clock, AlertTriangle, Plus } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import type { Gap } from '$lib/types';

  let { open = $bindable(false), gaps = $bindable<Gap[]>([]), onResolved = () => {} } = $props<{
    open: boolean;
    gaps: Gap[];
    onResolved?: () => void;
  }>();

  let filling = $state(false);
  let error = $state('');
  let selectedGapIndex = $state<number | null>(null);
  let fillTask = $state('');
  let fillPerson = $state('');
  let fillNote = $state('');

  function formatTime(isoString: string): string {
    return new Date(isoString).toLocaleTimeString('zh-TW', {
      hour: '2-digit',
      minute: '2-digit',
    });
  }

  function formatDuration(minutes: number): string {
    const hours = Math.floor(minutes / 60);
    const mins = minutes % 60;
    if (hours > 0) {
      return mins > 0 ? $t('time.hours_minutes', { hours, minutes: mins }) : $t('time.hours', { hours });
    }
    return $t('time.minutes', { minutes: mins });
  }

  function startFill(index: number) {
    selectedGapIndex = index;
    fillTask = '';
    fillPerson = '';
    fillNote = '';
  }

  function cancelFill() {
    selectedGapIndex = null;
  }

  async function handleFill(gap: Gap) {
    if (!fillTask.trim()) {
      error = $t('errors.task_title_required');
      return;
    }

    filling = true;
    error = '';

    try {
      await invoke('fill_gap', {
        startTime: gap.start_time,
        endTime: gap.end_time,
        taskTitle: fillTask.trim(),
        person: fillPerson.trim() || null,
        note: fillNote.trim() || null,
      });

      gaps = gaps.filter((_, i) => i !== selectedGapIndex);
      selectedGapIndex = null;
      fillTask = '';
      fillPerson = '';
      fillNote = '';

      if (gaps.length === 0) {
        onResolved();
        open = false;
      }
    } catch (err: any) {
      error = err.toString();
    } finally {
      filling = false;
    }
  }

  function handleSkipAll() {
    onResolved();
    open = false;
  }
</script>

{#if open}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
    <!-- Backdrop -->
    <div class="absolute inset-0 bg-black/50" />

    <!-- Dialog -->
    <div class="relative w-full max-w-2xl bg-background rounded-lg shadow-xl max-h-[80vh] flex flex-col">
      <!-- Header -->
      <div class="flex items-center justify-between px-6 py-4 border-b border-border">
        <div>
          <h2 class="text-lg font-semibold text-foreground flex items-center gap-2">
            <AlertTriangle size={20} class="text-amber-600" />
            {$t('eod_review.title')}
          </h2>
          <p class="mt-1 text-sm text-muted-foreground">
            {$t('eod_review.message', { count: gaps.length })}
          </p>
        </div>
        <button
          onclick={handleSkipAll}
          class="p-2 hover:bg-muted rounded-md transition-colors"
          aria-label={$t('actions.close')}
        >
          <X size={20} class="text-muted-foreground" />
        </button>
      </div>

      <!-- Content -->
      <div class="flex-1 overflow-y-auto px-6 py-4 space-y-3">
        {#each gaps as gap, index}
          <div class="border border-border rounded-lg p-4 space-y-3">
            <!-- Gap Info -->
            <div class="flex items-start justify-between">
              <div class="flex items-center gap-2 text-sm">
                <Clock size={16} class="text-amber-600" />
                <span class="font-medium text-foreground">
                  {formatTime(gap.start_time)} - {formatTime(gap.end_time)}
                </span>
                <span class="text-amber-600 font-medium">
                  {$t('timeline.gap_unrecorded', { minutes: gap.duration_minutes })}
                </span>
              </div>

              {#if selectedGapIndex !== index}
                <button
                  onclick={() => startFill(index)}
                  class="px-3 py-1 text-xs font-medium bg-amber-500/10 text-amber-600 hover:bg-amber-500/20 rounded-md transition-colors flex items-center gap-1"
                >
                  <Plus size={14} />
                  {$t('actions.fill_gap')}
                </button>
              {/if}
            </div>

            <!-- Fill Form -->
            {#if selectedGapIndex === index}
              <div class="space-y-2 pt-2 border-t border-border">
                <input
                  type="text"
                  bind:value={fillTask}
                  placeholder={$t('placeholders.task_title')}
                  class="w-full px-3 py-2 bg-background border border-input rounded-md text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
                  autofocus
                />
                <input
                  type="text"
                  bind:value={fillPerson}
                  placeholder={$t('placeholders.person')}
                  class="w-full px-3 py-2 bg-background border border-input rounded-md text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
                />
                <textarea
                  bind:value={fillNote}
                  placeholder={$t('placeholders.note')}
                  rows="2"
                  class="w-full px-3 py-2 bg-background border border-input rounded-md text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring resize-none"
                />
                <div class="flex gap-2">
                  <button
                    onclick={cancelFill}
                    disabled={filling}
                    class="px-3 py-1.5 text-sm font-medium text-muted-foreground hover:bg-muted rounded-md transition-colors disabled:opacity-50"
                  >
                    {$t('actions.cancel')}
                  </button>
                  <button
                    onclick={() => handleFill(gap)}
                    disabled={filling || !fillTask.trim()}
                    class="px-3 py-1.5 text-sm font-medium bg-primary text-primary-foreground hover:bg-primary/90 rounded-md transition-colors disabled:opacity-50"
                  >
                    {filling ? $t('actions.saving') : $t('actions.fill')}
                  </button>
                </div>
              </div>
            {/if}
          </div>
        {/each}

        {#if error}
          <div class="px-3 py-2 bg-destructive/10 border border-destructive/20 rounded-md text-sm text-destructive">
            {error}
          </div>
        {/if}
      </div>

      <!-- Footer -->
      <div class="px-6 py-4 border-t border-border flex justify-end gap-2">
        <button
          onclick={handleSkipAll}
          class="px-4 py-2 text-sm font-medium text-muted-foreground hover:bg-muted rounded-md transition-colors"
        >
          {$t('actions.skip_all')}
        </button>
      </div>
    </div>
  </div>
{/if}
