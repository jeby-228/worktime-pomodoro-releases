<script lang="ts">
  import { onMount } from 'svelte';
  import { Briefcase, Zap, Users, Coffee, AlertTriangle, ChevronDown } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  
  interface TimeSlot {
    id: string;
    task_name: string;
    slot_type: 'work' | 'interrupt' | 'meeting' | 'lunch';
    start_ts: number;
    end_ts: number | null;
    note: string | null;
    duration_minutes: number;
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
  let showGapReview = $state(false);
  
  const slotTypeIcons = {
    work: Briefcase,
    interrupt: Zap,
    meeting: Users,
    lunch: Coffee,
  };
  
  function formatTime(ts: number): string {
    const date = new Date(ts * 1000);
    return date.toLocaleTimeString('zh-TW', { hour: '2-digit', minute: '2-digit', hour12: false });
  }
  
  function formatDuration(minutes: number): string {
    const hours = Math.floor(minutes / 60);
    const mins = minutes % 60;
    if (hours > 0) {
      return `${hours}${$t('time.hour_short')} ${mins}${$t('time.minute_short')}`;
    }
    return `${mins} ${$t('time.minute_short')}`;
  }
  
  onMount(() => {
    loadToday();
  });
  
  async function loadToday() {
    // TODO: invoke backend commands
    console.log('Loading today slots');
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
    {#if gaps.length > 0}
      <button 
        onclick={() => showGapReview = !showGapReview}
        class="flex items-center gap-2 rounded-md bg-warning/10 px-3 py-1.5 text-sm font-medium text-warning hover:bg-warning/20"
      >
        <AlertTriangle class="size-4" />
        {$t('today.gaps')} ({gaps.length})
        <ChevronDown class="size-4" class:rotate-180={showGapReview} />
      </button>
    {/if}
  </div>
  
  <!-- Stats Summary -->
  <div class="grid grid-cols-4 gap-4 border-b border-border px-6 py-4">
    <div>
      <div class="text-xs text-muted-foreground">{$t('today.work_time')}</div>
      <div class="text-2xl font-semibold tabular-nums">{formatDuration(stats.work_minutes)}</div>
    </div>
    <div>
      <div class="text-xs text-muted-foreground">{$t('today.interrupt_time')}</div>
      <div class="text-2xl font-semibold tabular-nums">{formatDuration(stats.interrupt_minutes)}</div>
    </div>
    <div>
      <div class="text-xs text-muted-foreground">{$t('today.meeting_time')}</div>
      <div class="text-2xl font-semibold tabular-nums">{formatDuration(stats.meeting_minutes)}</div>
    </div>
    <div>
      <div class="text-xs text-muted-foreground">{$t('common.loading')}</div>
      <div class="text-2xl font-semibold tabular-nums">{formatDuration(stats.total_recorded)}</div>
    </div>
  </div>
  
  <!-- Gap Review (collapsible) -->
  {#if showGapReview && gaps.length > 0}
    <div class="border-b border-border bg-warning/5 px-6 py-3">
      <div class="space-y-2">
        {#each gaps as gap}
          <div class="flex items-center justify-between text-sm">
            <span class="text-muted-foreground">
              {formatTime(gap.start_ts)} - {formatTime(gap.end_ts)}
              <span class="ml-2 text-warning">({gap.minutes} {$t('time.minute_short')})</span>
            </span>
            <button 
              onclick={() => handleFillGap(gap)}
              class="rounded px-2 py-1 text-xs font-medium text-warning hover:bg-warning/10"
            >
              {$t('today.fill_gap')}
            </button>
          </div>
        {/each}
      </div>
    </div>
  {/if}
  
  <!-- Timeline -->
  <div class="flex-1 overflow-y-auto px-6 py-4">
    {#if slots.length === 0}
      <div class="flex h-full items-center justify-center text-center">
        <div class="text-sm text-muted-foreground">
          {$t('today.no_slots')}
        </div>
      </div>
    {:else}
      <div class="relative space-y-3 border-l-2 border-border pl-6">
        {#each slots as slot}
          <button
            onclick={() => handleSlotClick(slot)}
            class="relative block w-full text-left transition-colors hover:bg-accent/50 rounded-md -ml-6 pl-6 py-2"
          >
            <!-- Timeline dot -->
            <div class="absolute -left-[9px] top-3 size-4 rounded-full border-2 border-background
                        {slot.slot_type === 'work' ? 'bg-primary' : ''}
                        {slot.slot_type === 'interrupt' ? 'bg-accent' : ''}
                        {slot.slot_type === 'meeting' ? 'bg-muted-foreground' : ''}
                        {slot.slot_type === 'lunch' ? 'bg-muted' : ''}">
            </div>
            
            <div class="flex items-start justify-between gap-4">
              <div class="flex-1">
                <div class="flex items-center gap-2 text-sm font-medium">
                  <svelte:component this={slotTypeIcons[slot.slot_type]} class="size-4" />
                  {slot.task_name}
                  {#if !slot.end_ts}
                    <span class="rounded bg-accent px-1.5 py-0.5 text-xs text-accent-foreground">
                      {$t('today.ongoing')}
                    </span>
                  {/if}
                </div>
                <div class="mt-1 text-xs text-muted-foreground">
                  {formatTime(slot.start_ts)} - {slot.end_ts ? formatTime(slot.end_ts) : '...'} · {formatDuration(slot.duration_minutes)}
                </div>
                {#if slot.note}
                  <div class="mt-1 text-xs text-muted-foreground italic">{slot.note}</div>
                {/if}
              </div>
            </div>
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>
