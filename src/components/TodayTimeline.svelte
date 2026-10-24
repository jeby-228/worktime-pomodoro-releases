<script lang="ts">
  import { onMount } from 'svelte';
  import { Briefcase, Zap, Users, Coffee, Clock, AlertTriangle } from 'lucide-svelte';
  import SlotEditDrawer from './SlotEditDrawer.svelte';
  import { t } from '$lib/i18n';
  import { invoke } from '@tauri-apps/api/core';
  
  interface TimeSlot {
    id: string | number;
    task_name: string;
    slot_type: 'work' | 'interrupt' | 'meeting' | 'lunch';
    start_ts: number | string;
    end_ts: number | string | null;
    note: string | null;
    duration_minutes: number;
    parent_id: string | number | null;
  }
  
  interface Gap {
    start_ts: number | string;
    end_ts: number | string;
    minutes: number;
  }
  
  interface DayStats {
    work_minutes: number;
    interrupt_minutes: number;
    meeting_minutes: number;
    lunch_minutes: number;
    total_recorded: number;
  }
  
  let slots = $state<TimeSlot[]>([]);
  let gaps = $state<Gap[]>([]);
  let fillingGap = $state<Gap | null>(null);
  let fillTask = $state('');
  let fillError = $state('');
  let savingFill = $state(false);
  let editingSlot = $state<TimeSlot | null>(null);
  let editOpen = $state(false);
  let stats = $state<DayStats>({
    work_minutes: 0,
    interrupt_minutes: 0,
    meeting_minutes: 0,
    lunch_minutes: 0,
    total_recorded: 0,
  });
  
  const slotTypeIcons = {
    work: Briefcase,
    interrupt: Zap,
    meeting: Users,
    lunch: Coffee,
  };
  
  // Build display list with gaps interspersed
  let displayItems = $derived.by(() => {
    const items: Array<{ type: 'slot' | 'gap'; data: TimeSlot | Gap; isNested?: boolean }> = [];
    const allSlots = [...slots].sort((a, b) => tsValue(a.start_ts) - tsValue(b.start_ts));
    const allGaps = [...gaps].sort((a, b) => tsValue(a.start_ts) - tsValue(b.start_ts));
    
    let gapIndex = 0;
    for (const slot of allSlots) {
      // Insert gaps before this slot
      while (gapIndex < allGaps.length && tsValue(allGaps[gapIndex].end_ts) <= tsValue(slot.start_ts)) {
        items.push({ type: 'gap', data: allGaps[gapIndex] });
        gapIndex++;
      }
      
      // Skip interrupt slots (they'll be nested under parent)
      if (slot.parent_id) continue;
      
      items.push({ type: 'slot', data: slot });
      
      // Find and nest interrupts under this parent
      const interrupts = allSlots.filter(s => s.parent_id === slot.id);
      for (const interrupt of interrupts) {
        items.push({ type: 'slot', data: interrupt, isNested: true });
      }
    }
    
    // Add remaining gaps
    while (gapIndex < allGaps.length) {
      items.push({ type: 'gap', data: allGaps[gapIndex] });
      gapIndex++;
    }
    
    return items;
  });
  
  function tsValue(ts: number | string): number {
    if (typeof ts === 'number') return ts < 1e12 ? ts * 1000 : ts;
    const numeric = Number(ts);
    if (ts.trim() !== '' && !Number.isNaN(numeric)) return numeric < 1e12 ? numeric * 1000 : numeric;
    return new Date(ts).getTime();
  }

  function toRfc3339(ts: number | string): string {
    return new Date(tsValue(ts)).toISOString();
  }

  function formatTime(ts: number | string): string {
    const date = new Date(tsValue(ts));
    return date.toLocaleTimeString('zh-TW', { hour: '2-digit', minute: '2-digit', hour12: false });
  }

  function sameGap(a: Gap, b: Gap): boolean {
    return a.start_ts === b.start_ts && a.end_ts === b.end_ts;
  }
  
  function localDateString(date: Date): string {
    const month = String(date.getMonth() + 1).padStart(2, '0');
    const day = String(date.getDate()).padStart(2, '0');
    return `${date.getFullYear()}-${month}-${day}`;
  }

  function slotMinutes(slot: TimeSlot): number {
    const start = tsValue(slot.start_ts);
    if (!start) return 0;
    const end = slot.end_ts ? tsValue(slot.end_ts) : Date.now();
    if (!end) return 0;
    return Math.max(0, Math.round((end - start) / 60000));
  }

  function summarize(list: TimeSlot[]): DayStats {
    const next: DayStats = {
      work_minutes: 0,
      interrupt_minutes: 0,
      meeting_minutes: 0,
      lunch_minutes: 0,
      total_recorded: 0,
    };
    for (const slot of list) {
      const minutes = slotMinutes(slot);
      if (slot.slot_type === 'interrupt') next.interrupt_minutes += minutes;
      else if (slot.slot_type === 'meeting') next.meeting_minutes += minutes;
      else if (slot.slot_type === 'lunch') next.lunch_minutes += minutes;
      else next.work_minutes += minutes;
      next.total_recorded += minutes;
    }
    return next;
  }

  function formatDuration(minutes: number): string {
    const safeMinutes = minutes || 0;
    const hours = Math.floor(safeMinutes / 60);
    const mins = safeMinutes % 60;
    if (hours > 0) {
      return `${hours}${$t('time.hour_short')} ${mins}${$t('time.minute_short')}`;
    }
    return `${mins} ${$t('time.minute_short')}`;
  }
  
  onMount(() => {
    loadToday();
    window.addEventListener('task-started', loadToday);
    
    // Refresh every 30 seconds
    const interval = setInterval(loadToday, 30000);
    return () => {
      clearInterval(interval);
      window.removeEventListener('task-started', loadToday);
    };
  });
  
  async function loadToday() {
    try {
      const today = localDateString(new Date());
      const lookback = localDateString(new Date(Date.now() - 30 * 24 * 60 * 60 * 1000));
      const ranged = await invoke<TimeSlot[]>('get_slots_for_range', {
        request: { startDate: lookback, endDate: today },
      });
      slots = ranged.filter((slot) => {
        if (!slot.end_ts) return true;
        return localDateString(new Date(tsValue(slot.start_ts))) === today;
      });
      stats = summarize(slots);

      const todayGaps = await invoke<Gap[]>('find_gaps', { date: today });
      gaps = todayGaps;
    } catch (err) {
      console.error('Failed to load today data:', err);
    }
  }
  
  function handleSlotClick(slot: TimeSlot) {
    editingSlot = slot;
    editOpen = true;
  }
  
  function handleFillGap(gap: Gap) {
    fillingGap = gap;
    fillTask = '';
    fillError = '';
  }

  function cancelFill() {
    fillingGap = null;
    fillError = '';
  }

  async function submitFill() {
    if (!fillingGap) return;
    if (!fillTask.trim()) {
      fillError = $t('errors.task_title_required');
      return;
    }

    savingFill = true;
    fillError = '';
    try {
      await invoke('fill_gap', {
        request: {
          start_time: toRfc3339(fillingGap.start_ts),
          end_time: toRfc3339(fillingGap.end_ts),
          task_title: fillTask.trim(),
          person: null,
          note: null,
        },
      });
      fillingGap = null;
      await loadToday();
    } catch (err) {
      fillError = String(err);
    } finally {
      savingFill = false;
    }
  }
</script>

<div class="flex h-full flex-col rounded-lg border border-border bg-card text-card-foreground shadow-sm">
  <!-- Header -->
  <div class="flex items-center justify-between border-b border-border px-6 py-4">
    <h2 class="text-lg font-semibold">{$t('today.title')}</h2>
    <div class="text-sm text-muted-foreground">
      {new Date().toLocaleDateString('zh-TW', { month: '2-digit', day: '2-digit', weekday: 'short' })}
    </div>
  </div>
  
  <!-- Stats Summary (one-line totals) -->
  <div class="flex items-center gap-6 border-b border-border px-6 py-3 text-sm">
    <div class="flex items-center gap-2">
      <span class="text-muted-foreground">{$t('today.total')}</span>
      <span class="font-semibold tabular-nums">{formatDuration(stats.total_recorded)}</span>
    </div>
    <div class="h-4 w-px bg-border"></div>
    <div class="flex items-center gap-2">
      <Briefcase class="size-4 text-muted-foreground" />
      <span class="tabular-nums">{formatDuration(stats.work_minutes)}</span>
    </div>
    <div class="flex items-center gap-2">
      <Zap class="size-4 text-muted-foreground" />
      <span class="tabular-nums">{formatDuration(stats.interrupt_minutes)}</span>
    </div>
    <div class="flex items-center gap-2">
      <Users class="size-4 text-muted-foreground" />
      <span class="tabular-nums">{formatDuration(stats.meeting_minutes)}</span>
    </div>
    <div class="flex items-center gap-2">
      <Coffee class="size-4 text-muted-foreground" />
      <span class="tabular-nums">{formatDuration(stats.lunch_minutes)}</span>
    </div>
  </div>
  
  <!-- Timeline List (shadcn Item style) -->
  <div class="flex-1 overflow-y-auto">
    {#if displayItems.length === 0}
      <div class="flex h-full items-center justify-center text-center">
        <div class="text-sm text-muted-foreground">
          {$t('today.no_slots')}
        </div>
      </div>
    {:else}
      <div class="divide-y divide-border">
        {#each displayItems as item (`${item.type}-${item.type === 'slot' ? (item.data as TimeSlot).id : String((item.data as Gap).start_ts)}`)}
          {#if item.type === 'slot'}
            {@const slot = item.data as TimeSlot}
            {@const Icon = slotTypeIcons[slot.slot_type]}
            <button
              onclick={() => handleSlotClick(slot)}
              class="flex w-full items-center gap-3 px-6 py-3 text-left transition-colors hover:bg-accent/50 {item.isNested ? 'pl-12' : ''}"
            >
              <!-- Icon box -->
              <div class="flex size-10 flex-shrink-0 items-center justify-center rounded-md bg-muted">
                <Icon class="size-5" />
              </div>
              
              <!-- Content -->
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                  <div class="font-medium truncate">{slot.task_name}</div>
                  {#if !slot.end_ts}
                    <span class="flex-shrink-0 rounded bg-accent px-2 py-0.5 text-xs font-medium text-accent-foreground">
                      {$t('today.ongoing')}
                    </span>
                  {/if}
                  {#if item.isNested}
                    <span class="flex-shrink-0 rounded bg-accent px-2 py-0.5 text-xs font-medium text-accent-foreground">
                      {$t('slot_type.interrupt')}
                    </span>
                  {/if}
                </div>
                <div class="mt-1 text-sm text-muted-foreground">
                  {formatTime(slot.start_ts)} - {slot.end_ts ? formatTime(slot.end_ts) : '...'} · {formatDuration(slotMinutes(slot))}
                </div>
              </div>
            </button>
          {:else}
            {@const gap = item.data as Gap}
            <div class="border-dashed px-6 py-3">
            <div class="flex items-center gap-3">
              <!-- Icon box (dashed) -->
              <div class="flex size-10 flex-shrink-0 items-center justify-center rounded-md border-2 border-dashed border-warning/50">
                <AlertTriangle class="size-5 text-warning" />
              </div>
              
              <!-- Content -->
              <div class="flex-1 min-w-0">
                <div class="font-medium text-warning">
                  {$t('today.gaps')} {gap.minutes} {$t('time.minute_short')}
                </div>
                <div class="mt-1 text-sm text-muted-foreground">
                  {formatTime(gap.start_ts)} - {formatTime(gap.end_ts)}
                </div>
              </div>
              
              <!-- Action -->
              <button
                onclick={() => handleFillGap(gap)}
                class="flex-shrink-0 rounded px-3 py-1.5 text-sm font-medium text-warning hover:bg-warning/10"
              >
                {$t('today.fill_gap')}
              </button>
            </div>
            {#if fillingGap && sameGap(fillingGap, gap)}
              <div class="mt-3 space-y-2 pl-12">
                <input
                  type="text"
                  bind:value={fillTask}
                  placeholder={$t('placeholders.task_title')}
                  class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
                />
                {#if fillError}
                  <div class="text-sm text-destructive">{fillError}</div>
                {/if}
                <div class="flex gap-2">
                  <button
                    onclick={cancelFill}
                    disabled={savingFill}
                    class="rounded-md px-3 py-1.5 text-sm font-medium text-muted-foreground hover:bg-muted disabled:opacity-50"
                  >
                    {$t('actions.cancel')}
                  </button>
                  <button
                    onclick={submitFill}
                    disabled={savingFill}
                    class="rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
                  >
                    {savingFill ? $t('actions.saving') : $t('actions.fill')}
                  </button>
                </div>
              </div>
            {/if}
            </div>
          {/if}
        {/each}
      </div>
    {/if}
  </div>
</div>

<SlotEditDrawer bind:open={editOpen} bind:slot={editingSlot} onSaved={loadToday} />
