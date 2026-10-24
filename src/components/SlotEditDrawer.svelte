<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { X, Clock, User, FileText, Calendar, Tag } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import type { TimeSlot } from '$lib/types';

  let { open = $bindable(false), slot = $bindable<TimeSlot | null>(null), onSaved = () => {} } = $props<{
    open: boolean;
    slot: TimeSlot | null;
    onSaved?: () => void;
  }>();

  const openingTitle = slot?.task_name || slot?.task_title || '';
  const openingStart = toLocalInput(slot?.start_time ?? slot?.start_ts);
  const openingEnd = toLocalInput(slot?.end_time ?? slot?.end_ts);
  let taskTitle = $state(openingTitle);
  let person = $state(slot?.person || '');
  let note = $state(slot?.note || '');
  let startTime = $state(openingStart);
  let endTime = $state(openingEnd);
  let slotType = $state<'work' | 'interrupt' | 'meeting' | 'lunch'>(slot?.slot_type ?? 'work');
  let splitAt = $state(midpoint(openingStart, openingEnd));
  let secondTitle = $state(openingTitle);
  let saving = $state(false);
  let error = $state('');

  function midpoint(start: string, end: string): string {
    const a = new Date(start).getTime();
    const b = new Date(end).getTime();
    if (Number.isNaN(a) || Number.isNaN(b) || b <= a) return '';
    return toLocalInput(new Date(Math.floor((a + b) / 2)).toISOString());
  }

  function slotInstant(value: string | number | null | undefined): Date | null {
    if (value == null || value === '') return null;
    if (typeof value === 'number') {
      return new Date(value < 1e12 ? value * 1000 : value);
    }
    const date = new Date(value);
    return Number.isNaN(date.getTime()) ? null : date;
  }

  function toLocalInput(value: string | number | null | undefined): string {
    const date = slotInstant(value);
    if (!date) return '';
    const pad = (n: number) => String(n).padStart(2, '0');
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
  }

  function close() {
    open = false;
    slot = null;
    error = '';
  }

  async function handleSave() {
    if (!slot) return;
    if (!taskTitle.trim()) {
      error = $t('errors.task_title_required');
      return;
    }
    if (!startTime) {
      error = $t('errors.start_time_required');
      return;
    }

    saving = true;
    error = '';

    try {
      await invoke('update_slot_manual', {
        request: {
          id: Number(slot.id),
          task_name: taskTitle.trim(),
          slot_type: slotType,
          start_ts: new Date(startTime).toISOString(),
          end_ts: endTime ? new Date(endTime).toISOString() : null,
          note: note.trim() || null,
        },
      });
      window.dispatchEvent(new CustomEvent('task-started'));
      onSaved();
      close();
    } catch (err: any) {
      error = err.toString();
    } finally {
      saving = false;
    }
  }

  async function handleSplit() {
    if (!slot?.id || !splitAt) return;
    const cut = new Date(splitAt).getTime();
    const start = new Date(startTime).getTime();
    const end = new Date(endTime).getTime();
    if (Number.isNaN(cut) || Number.isNaN(start) || Number.isNaN(end) || cut <= start || cut >= end) {
      error = $t('errors.split_outside');
      return;
    }
    saving = true;
    error = '';
    try {
      await invoke('split_slot', {
        request: {
          id: Number(slot.id),
          at: new Date(splitAt).toISOString(),
          second_task_name: secondTitle.trim() || null,
        },
      });
      window.dispatchEvent(new CustomEvent('task-started'));
      onSaved();
      close();
    } catch (err: any) {
      error = err.toString();
    } finally {
      saving = false;
    }
  }

  async function handleDelete() {
    if (!slot || !confirm($t('confirm.delete_slot'))) return;

    saving = true;
    error = '';

    try {
      await invoke('delete_slot', { slotId: Number(slot.id) });
      onSaved();
      close();
    } catch (err: any) {
      error = err.toString();
    } finally {
      saving = false;
    }
  }
</script>

{#if open}
  <div class="fixed inset-0 z-50 flex">
    <!-- Backdrop -->
    <button
      type="button"
      class="absolute inset-0 bg-black/50"
      onclick={close}
      aria-label={$t('actions.close')}
    ></button>

    <!-- Drawer -->
    <div class="ml-auto relative w-full max-w-md bg-background shadow-xl flex flex-col h-full">
      <!-- Header -->
      <div class="flex items-center justify-between px-6 py-4 border-b border-border">
        <h2 class="text-lg font-semibold text-foreground">
          {$t('slot_edit.title')}
        </h2>
        <button
          onclick={close}
          class="p-2 hover:bg-muted rounded-md transition-colors"
          aria-label={$t('actions.close')}
        >
          <X size={20} class="text-muted-foreground" />
        </button>
      </div>

      <!-- Content -->
      <div class="flex-1 overflow-y-auto px-6 py-4 space-y-4">
        <!-- Slot Type -->
        <div class="space-y-2">
          <label class="text-sm font-medium text-foreground flex items-center gap-2">
            <Tag size={16} class="text-muted-foreground" />
            {$t('slot_edit.type')}
          </label>
          <div class="grid grid-cols-2 gap-2">
            {#each [
              { value: 'work', label: $t('slot_types.work') },
              { value: 'interrupt', label: $t('slot_types.interrupt') },
              { value: 'meeting', label: $t('slot_types.meeting') },
              { value: 'lunch', label: $t('slot_types.lunch') }
            ] as type (type.value)}
              <button
                onclick={() => slotType = type.value as typeof slotType}
                class="px-3 py-2 rounded-md text-sm font-medium transition-colors {slotType === type.value
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-muted text-muted-foreground hover:bg-muted/80'}"
              >
                {type.label}
              </button>
            {/each}
          </div>
        </div>

        <!-- Task Title -->
        <div class="space-y-2">
          <label for="task-title" class="text-sm font-medium text-foreground flex items-center gap-2">
            <FileText size={16} class="text-muted-foreground" />
            {$t('slot_edit.task_title')}
          </label>
          <input
            id="task-title"
            type="text"
            bind:value={taskTitle}
            class="w-full px-3 py-2 bg-background border border-input rounded-md text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
            placeholder={$t('placeholders.task_title')}
          />
        </div>

        <!-- Person -->
        <div class="space-y-2">
          <label for="person" class="text-sm font-medium text-foreground flex items-center gap-2">
            <User size={16} class="text-muted-foreground" />
            {$t('slot_edit.person')}
          </label>
          <input
            id="person"
            type="text"
            bind:value={person}
            class="w-full px-3 py-2 bg-background border border-input rounded-md text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
            placeholder={$t('placeholders.person')}
          />
        </div>

        <!-- Time Range -->
        <div class="grid grid-cols-2 gap-3">
          <div class="space-y-2">
            <label for="start-time" class="text-sm font-medium text-foreground flex items-center gap-2">
              <Clock size={16} class="text-muted-foreground" />
              {$t('slot_edit.start_time')}
            </label>
            <input
              id="start-time"
              type="datetime-local"
              bind:value={startTime}
              class="w-full px-3 py-2 bg-background border border-input rounded-md text-foreground text-sm focus:outline-none focus:ring-2 focus:ring-ring"
            />
          </div>

          <div class="space-y-2">
            <label for="end-time" class="text-sm font-medium text-foreground flex items-center gap-2">
              <Clock size={16} class="text-muted-foreground" />
              {$t('slot_edit.end_time')}
            </label>
            <input
              id="end-time"
              type="datetime-local"
              bind:value={endTime}
              class="w-full px-3 py-2 bg-background border border-input rounded-md text-foreground text-sm focus:outline-none focus:ring-2 focus:ring-ring"
            />
          </div>
        </div>

        {#if endTime}
          <div class="space-y-2 rounded-md border border-border p-3">
            <div class="text-sm font-medium text-foreground">{$t('slot_edit.split')}</div>
            <label for="split-at" class="text-sm text-muted-foreground">{$t('slot_edit.split_at')}</label>
            <input
              id="split-at"
              type="datetime-local"
              bind:value={splitAt}
              class="w-full px-3 py-2 bg-background border border-input rounded-md text-foreground text-sm focus:outline-none focus:ring-2 focus:ring-ring"
            />
            <label for="second-task" class="text-sm text-muted-foreground">{$t('slot_edit.second_task')}</label>
            <input
              id="second-task"
              type="text"
              bind:value={secondTitle}
              class="w-full px-3 py-2 bg-background border border-input rounded-md text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
              placeholder={$t('placeholders.task_title')}
            />
            <button
              onclick={handleSplit}
              disabled={saving}
              class="px-3 py-1.5 text-sm font-medium rounded-md border border-border hover:bg-muted disabled:opacity-50"
            >
              {$t('slot_edit.split')}
            </button>
          </div>
        {/if}

        <!-- Note -->
        <div class="space-y-2">
          <label for="note" class="text-sm font-medium text-foreground flex items-center gap-2">
            <FileText size={16} class="text-muted-foreground" />
            {$t('slot_edit.note')}
          </label>
          <textarea
            id="note"
            bind:value={note}
            rows="3"
            class="w-full px-3 py-2 bg-background border border-input rounded-md text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring resize-none"
            placeholder={$t('placeholders.note')}
          ></textarea>
        </div>

        <!-- Error -->
        {#if error}
          <div class="px-3 py-2 bg-destructive/10 border border-destructive/20 rounded-md text-sm text-destructive">
            {error}
          </div>
        {/if}
      </div>

      <!-- Footer -->
      <div class="px-6 py-4 border-t border-border flex items-center justify-between">
        <button
          onclick={handleDelete}
          disabled={saving}
          class="px-4 py-2 text-sm font-medium text-destructive hover:bg-destructive/10 rounded-md transition-colors disabled:opacity-50"
        >
          {$t('actions.delete')}
        </button>

        <div class="flex gap-2">
          <button
            onclick={close}
            disabled={saving}
            class="px-4 py-2 text-sm font-medium text-muted-foreground hover:bg-muted rounded-md transition-colors disabled:opacity-50"
          >
            {$t('actions.cancel')}
          </button>
          <button
            onclick={handleSave}
            disabled={saving}
            class="px-4 py-2 text-sm font-medium bg-primary text-primary-foreground hover:bg-primary/90 rounded-md transition-colors disabled:opacity-50"
          >
            {saving ? $t('actions.saving') : $t('actions.save')}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
