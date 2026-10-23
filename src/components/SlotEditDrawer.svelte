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

  let taskTitle = $state('');
  let person = $state('');
  let note = $state('');
  let startTime = $state('');
  let endTime = $state('');
  let slotType = $state<'work' | 'interrupt' | 'meeting' | 'lunch'>('work');
  let saving = $state(false);
  let error = $state('');

  $effect(() => {
    if (open && slot) {
      taskTitle = slot.task_title || '';
      person = slot.person || '';
      note = slot.note || '';
      startTime = new Date(slot.start_time).toISOString().slice(0, 16);
      endTime = slot.end_time ? new Date(slot.end_time).toISOString().slice(0, 16) : '';
      slotType = slot.slot_type;
      error = '';
    }
  });

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
      const updatedSlot = {
        id: slot.id,
        slot_type: slotType,
        task_title: taskTitle.trim(),
        person: person.trim() || null,
        note: note.trim() || null,
        start_time: new Date(startTime).toISOString(),
        end_time: endTime ? new Date(endTime).toISOString() : null,
        parent_id: slot.parent_id,
      };

      await invoke('update_slot_manual', { slot: updatedSlot });
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
      await invoke('delete_slot', { slotId: slot.id });
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
      class="absolute inset-0 bg-black/50"
      onclick={close}
      aria-label={$t('actions.close')}
    />

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
            ] as type}
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
          />
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
