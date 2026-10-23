<script lang="ts">
  import { onMount } from 'svelte';
  import { Briefcase, Zap, Users, Coffee, Clock, AlertTriangle } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import { invoke } from '@tauri-apps/api/core';
  
  interface TimeSlot {
    id: string;
    task_name: string;
    slot_type: 'work' | 'interrupt' | 'meeting' | 'lunch';
    start_ts: number;
    end_ts: number | null;
    note: string | null;
    duration_minutes: number;
    parent_id: string | null;
  }
  
  interface Gap {
    start_ts: number;
    end_ts: number;
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
  let displayItems = $derived(() => {
    const items: Array<{ type: 'slot' | 'gap'; data: TimeSlot | Gap; isNested?: boolean }> = [];
    const allSlots = [...slots].sort((a, b) => a.start_ts - b.start_ts);
    const allGaps = [...gaps].sort((a, b) => a.start_ts - b.start_ts);
    
    let gapIndex = 0;
    for (const slot of allSlots) {
      // Insert gaps before this slot
      while (gapIndex < allGaps.length && allGaps[gapIndex].end_ts <= slot.start_ts) {
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
  
  function formatTime(ts: number): string {
    const date = new Date(ts * 1000);
    return date.toLocaleTimeString('zh-TW', { hour: '2-digit', minute: '2-digit', hour12: false });
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
    
    // Refresh every 30 seconds
    const interval = setInterval(loadToday, 30000);
    return () => clearInterval(interval);
  });
  
  async function loadToday() {
    try {
      const today = new Date().toISOString().split('T')[0];
      
      // Load today's slots
      const todaySlots = await invoke<TimeSlot[]>('get_slots_for_date', { date: today });
      slots = todaySlots;
      
      // Load day stats
      const dayStats = await invoke<DayStats>('get_day_stats', { date: today });
      stats = dayStats;
      
      // Load gaps
      const todayGaps = await invoke<Gap[]>('find_gaps', { date: today });
      gaps = todayGaps;
    } catch (err) {
      console.error('Failed to load today data:', err);
    }
  }
  
  function handleSlotClick(slot: TimeSlot) {
    // TODO: open edit drawer
    console.log('Edit slot', slot);
  }
  
  function handleFillGap(gap: Gap) {
    // TODO: open fill gap dialog
    console.log('Fill gap', gap);
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
    {#if displayItems().length === 0}
      <div class="flex h-full items-center justify-center text-center">
        <div class="text-sm text-muted-foreground">
          {$t('today.no_slots')}
        </div>
      </div>
    {:else}
      <div class="divide-y divide-border">
        {#each displayItems() as item}
          {#if item.type === 'slot'}
            {@const slot = item.data as TimeSlot}
            <button
              onclick={() => handleSlotClick(slot)}
              class="flex w-full items-center gap-3 px-6 py-3 text-left transition-colors hover:bg-accent/50"
              class:pl-12={item.isNested}
            >
              <!-- Icon box -->
              <div class="flex size-10 flex-shrink-0 items-center justify-center rounded-md bg-muted">
                <svelte:component this={slotTypeIcons[slot.slot_type]} class="size-5" />
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
                  {formatTime(slot.start_ts)} - {slot.end_ts ? formatTime(slot.end_ts) : '...'} · {formatDuration(slot.duration_minutes)}
                </div>
              </div>
            </button>
          {:else}
            {@const gap = item.data as Gap}
            <div class="flex items-center gap-3 border-dashed px-6 py-3">
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
          {/if}
        {/each}
      </div>
    {/if}
  </div>
</div>
