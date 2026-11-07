<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { X, Clock, User, FileText, Calendar, Tag } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import type { TimeSlot } from '$lib/types';
  import CategorySelect from './CategorySelect.svelte';

  const pad = (n: number) => String(n).padStart(2, '0');

  let { open = $bindable(false), slot = $bindable<TimeSlot | null>(null), onSaved = () => {} } = $props<{
    open: boolean;
    slot: TimeSlot | null;
    onSaved?: () => void;
  }>();

  // A slot without an id is a new record typed in by hand.
  const isNew = !slot?.id;
  const openingTitle = slot?.task_name || slot?.task_title || '';
  const openingStart = toLocalInput(slot?.start_time ?? slot?.start_ts);
  const openingEnd = toLocalInput(slot?.end_time ?? slot?.end_ts);
  let taskTitle = $state(openingTitle);
  let person = $state(slot?.person || '');
  let note = $state(slot?.note || '');
  let startTime = $state(openingStart);
  let endTime = $state(openingEnd);
  let slotType = $state<'work' | 'interrupt' | 'meeting' | 'lunch'>(slot?.slot_type ?? 'work');
  let categoryId = $state<number | null>(slot?.category_id ?? null);
  let saving = $state(false);
  let error = $state('');

  // Carving works on the slot as saved, not on unsaved edits above.
  const savedStartRaw = slot?.start_time ?? slot?.start_ts;
  const savedEndRaw = slot?.end_time ?? slot?.end_ts;
  const savedStart = slotInstant(savedStartRaw);
  const savedEnd = slotInstant(savedEndRaw);
  let carveFrom = $state(
    savedStart && savedEnd
      ? toClock(new Date(Math.floor((savedStart.getTime() + savedEnd.getTime()) / 2)))
      : '',
  );
  let carveTo = $state(savedEnd ? toClock(savedEnd) : '');
  let carveTitle = $state('');
  let carveType = $state<'work' | 'interrupt' | 'meeting' | 'lunch'>(slot?.slot_type ?? 'work');
  let carveCategory = $state<number | null>(slot?.category_id ?? null);

  function minutesBetween(a: Date, b: Date): number {
    return Math.round((b.getTime() - a.getTime()) / 60000);
  }

  function toClock(date: Date): string {
    return `${pad(date.getHours())}:${pad(date.getMinutes())}`;
  }

  /** Turn "HH:MM" into the instant inside the saved slot, which may cross midnight. */
  function resolveClock(clock: string): Date | null {
    if (!savedStart || !savedEnd || !/^\d{2}:\d{2}$/.test(clock)) return null;
    const [hours, minutes] = clock.split(':').map(Number);
    for (let day = 0; day <= 1; day += 1) {
      const candidate = new Date(savedStart);
      candidate.setDate(candidate.getDate() + day);
      candidate.setHours(hours, minutes, 0, 0);
      // The inputs only show minutes, so the start or end minute means the exact edge.
      if (Math.abs(candidate.getTime() - savedStart.getTime()) < 60_000) return savedStart;
      if (Math.abs(candidate.getTime() - savedEnd.getTime()) < 60_000) return savedEnd;
      if (candidate > savedStart && candidate < savedEnd) return candidate;
    }
    return null;
  }

  /** Send the edges exactly as stored, so they compare equal on the backend. */
  function carveInstant(date: Date): string {
    if (date === savedStart) return String(savedStartRaw);
    if (date === savedEnd) return String(savedEndRaw);
    return date.toISOString();
  }

  const carvePlan = $derived.by(() => {
    if (!savedStart || !savedEnd) return null;
    const from = resolveClock(carveFrom);
    const to = resolveClock(carveTo);
    const total = savedEnd.getTime() - savedStart.getTime();
    if (!from || !to || to <= from || total <= 0) return { from, to, valid: false, whole: false };
    return {
      from,
      to,
      valid: true,
      whole: from === savedStart && to === savedEnd,
      before: minutesBetween(savedStart, from),
      carved: minutesBetween(from, to),
      after: minutesBetween(to, savedEnd),
      left: ((from.getTime() - savedStart.getTime()) / total) * 100,
      width: ((to.getTime() - from.getTime()) / total) * 100,
    };
  });

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
    if (isNew && !endTime) {
      error = $t('errors.end_time_required');
      return;
    }
    if (endTime && new Date(endTime).getTime() <= new Date(startTime).getTime()) {
      error = $t('errors.end_before_start');
      return;
    }

    saving = true;
    error = '';

    try {
      await invoke(isNew ? 'create_slot_manual' : 'update_slot_manual', {
        request: {
          ...(isNew ? {} : { id: Number(slot.id) }),
          task_name: taskTitle.trim(),
          slot_type: slotType,
          start_ts: new Date(startTime).toISOString(),
          end_ts: endTime ? new Date(endTime).toISOString() : null,
          note: note.trim() || null,
          category_id: slotType === 'lunch' ? null : categoryId,
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

  async function handleCarve() {
    if (!slot?.id || !carvePlan) return;
    if (!carvePlan.valid || !carvePlan.from || !carvePlan.to) {
      error = $t('errors.carve_outside');
      return;
    }
    if (carvePlan.whole) {
      error = $t('errors.carve_whole');
      return;
    }
    saving = true;
    error = '';
    try {
      await invoke('carve_slot', {
        request: {
          id: Number(slot.id),
          from: carveInstant(carvePlan.from),
          to: carveInstant(carvePlan.to),
          task_name: carveTitle.trim(),
          slot_type: carveType,
          category_id: carveType === 'lunch' ? null : carveCategory,
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
          {isNew ? $t('slot_edit.new_title') : $t('slot_edit.title')}
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

        <!-- Category -->
        {#if slotType !== 'lunch'}
          <div class="space-y-2">
            <label for="slot-category" class="text-sm font-medium text-foreground flex items-center gap-2">
              <Tag size={16} class="text-muted-foreground" />
              {$t('categories.label')}
            </label>
            <CategorySelect id="slot-category" bind:value={categoryId} />
          </div>
        {/if}

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

        {#if !isNew && savedStart && savedEnd}
          <div class="space-y-3 rounded-md border border-border p-3">
            <div>
              <div class="text-sm font-medium text-foreground">{$t('slot_edit.carve')}</div>
              <p class="mt-0.5 text-xs text-muted-foreground">{$t('slot_edit.carve_hint')}</p>
            </div>

            <!-- Where the carved stretch sits inside the saved slot -->
            <div>
              <div class="relative h-3 overflow-hidden rounded-full bg-muted">
                {#if carvePlan?.valid}
                  <div
                    class="absolute inset-y-0 bg-primary"
                    style="left: {carvePlan.left}%; width: {carvePlan.width}%"
                  ></div>
                {/if}
              </div>
              <div class="mt-1 flex justify-between text-xs tabular-nums text-muted-foreground">
                <span>{toClock(savedStart)}</span>
                <span>{toClock(savedEnd)}</span>
              </div>
            </div>

            <div class="grid grid-cols-2 gap-3">
              <div class="space-y-1">
                <div class="flex items-center justify-between">
                  <label for="carve-from" class="text-sm text-muted-foreground">{$t('slot_edit.carve_from')}</label>
                  <button
                    type="button"
                    onclick={() => (carveFrom = toClock(savedStart))}
                    class="text-xs text-muted-foreground hover:text-foreground"
                  >
                    {$t('slot_edit.carve_from_start')}
                  </button>
                </div>
                <input
                  id="carve-from"
                  type="time"
                  bind:value={carveFrom}
                  class="w-full px-3 py-2 bg-background border border-input rounded-md text-foreground text-sm tabular-nums focus:outline-none focus:ring-2 focus:ring-ring"
                />
              </div>
              <div class="space-y-1">
                <div class="flex items-center justify-between">
                  <label for="carve-to" class="text-sm text-muted-foreground">{$t('slot_edit.carve_to')}</label>
                  <button
                    type="button"
                    onclick={() => (carveTo = toClock(savedEnd))}
                    class="text-xs text-muted-foreground hover:text-foreground"
                  >
                    {$t('slot_edit.carve_to_end')}
                  </button>
                </div>
                <input
                  id="carve-to"
                  type="time"
                  bind:value={carveTo}
                  class="w-full px-3 py-2 bg-background border border-input rounded-md text-foreground text-sm tabular-nums focus:outline-none focus:ring-2 focus:ring-ring"
                />
              </div>
            </div>

            {#if carvePlan?.valid && !carvePlan.whole}
              <p class="text-xs tabular-nums text-muted-foreground">
                {$t('slot_edit.carve_summary', {
                  before: carvePlan.before,
                  carved: carvePlan.carved,
                  after: carvePlan.after,
                })}
              </p>
            {:else}
              <p class="text-xs text-destructive">
                {carvePlan?.whole ? $t('errors.carve_whole') : $t('errors.carve_outside')}
              </p>
            {/if}

            <div class="grid grid-cols-4 gap-1.5">
              {#each ['work', 'interrupt', 'meeting', 'lunch'] as type (type)}
                <button
                  type="button"
                  onclick={() => (carveType = type as typeof carveType)}
                  aria-pressed={carveType === type}
                  class="px-2 py-1.5 rounded-md text-xs font-medium transition-colors {carveType === type
                    ? 'bg-primary text-primary-foreground'
                    : 'bg-muted text-muted-foreground hover:bg-muted/80'}"
                >
                  {$t(`slot_types.${type}`)}
                </button>
              {/each}
            </div>
            {#if carveType !== 'lunch'}
              <CategorySelect id="carve-category" bind:value={carveCategory} />
            {/if}
            <label for="carve-task" class="sr-only">{$t('slot_edit.carve_task')}</label>
            <input
              id="carve-task"
              type="text"
              bind:value={carveTitle}
              class="w-full px-3 py-2 bg-background border border-input rounded-md text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
              placeholder={$t('slot_edit.carve_task_placeholder', { name: openingTitle })}
            />
            <button
              onclick={handleCarve}
              disabled={saving || !carvePlan?.valid || carvePlan.whole}
              class="px-3 py-1.5 text-sm font-medium rounded-md border border-border hover:bg-muted disabled:opacity-50"
            >
              {$t('slot_edit.carve_action')}
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
        {#if isNew}
          <span></span>
        {:else}
          <button
            onclick={handleDelete}
            disabled={saving}
            class="px-4 py-2 text-sm font-medium text-destructive hover:bg-destructive/10 rounded-md transition-colors disabled:opacity-50"
          >
            {$t('actions.delete')}
          </button>
        {/if}

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
