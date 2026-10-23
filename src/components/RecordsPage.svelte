<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { List, Table as TableIcon, Filter, Download, Calendar, Briefcase, Zap, Users, Coffee, Clock, AlertTriangle } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import type { TimeSlot, DayStats } from '$lib/types';
  import SlotEditDrawer from './SlotEditDrawer.svelte';
  import EodGapReviewDialog from './EodGapReviewDialog.svelte';

  type ViewMode = 'list' | 'table';
  type GroupedSlots = Record<string, { slots: TimeSlot[]; stats: DayStats }>;

  let viewMode = $state<ViewMode>('list');
  let startDate = $state(getDateString(new Date(Date.now() - 7 * 24 * 60 * 60 * 1000)));
  let endDate = $state(getDateString(new Date()));
  let searchQuery = $state('');
  let filterType = $state<string | null>(null);
  
  let groupedData = $state<GroupedSlots>({});
  let taskSummary = $state<Array<{ task: string; minutes: number }>>([]);
  let loading = $state(true);

  let editingSlot = $state<TimeSlot | null>(null);
  let showEditDrawer = $state(false);
  let showFilterDialog = $state(false);
  let showExportDialog = $state(false);
  let showDatePicker = $state(false);

  const slotTypeIcons = {
    work: Briefcase,
    interrupt: Zap,
    meeting: Users,
    lunch: Coffee,
  };

  function getDateString(date: Date): string {
    return date.toISOString().split('T')[0];
  }

  function formatDate(isoDate: string): string {
    const date = new Date(isoDate);
    const weekdays = ['日', '一', '二', '三', '四', '五', '六'];
    return `${date.getMonth() + 1}/${date.getDate()} 週${weekdays[date.getDay()]}`;
  }

  function formatTime(isoString: string): string {
    return new Date(isoString).toLocaleTimeString('zh-TW', {
      hour: '2-digit',
      minute: '2-digit',
    });
  }

  function slotTitle(slot: TimeSlot): string {
    return slot.task_name || slot.task_title || '';
  }

  function slotInstant(value: string | number | null | undefined): string | null {
    if (value == null || value === '') return null;
    if (typeof value === 'number') {
      return new Date(value < 1e12 ? value * 1000 : value).toISOString();
    }
    return value;
  }

  function slotStart(slot: TimeSlot): string {
    return slotInstant(slot.start_time ?? slot.start_ts) || '';
  }

  function slotEnd(slot: TimeSlot): string | null {
    return slotInstant(slot.end_time ?? slot.end_ts);
  }

  function formatDuration(minutes: number): string {
    const hours = Math.floor(minutes / 60);
    const mins = minutes % 60;
    if (hours > 0) {
      return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
    }
    return `${mins}m`;
  }

  async function loadData() {
    loading = true;
    try {
      const slots: TimeSlot[] = await invoke('get_slots_for_range', {
        startDate,
        endDate,
      });

      // Group by date
      const grouped: GroupedSlots = {};
      for (const slot of slots) {
        const date = slotStart(slot).split('T')[0];
        if (!grouped[date]) {
          grouped[date] = { slots: [], stats: await invoke('get_day_stats', { date }) };
        }
        grouped[date].slots.push(slot);
      }
      groupedData = grouped;

      // Load task summary
      taskSummary = await invoke('get_task_summary', { startDate, endDate });
    } catch (err) {
      console.error('Failed to load records:', err);
    } finally {
      loading = false;
    }
  }

  function handleEditSlot(slot: TimeSlot) {
    editingSlot = slot;
    showEditDrawer = true;
  }

  function handleSlotSaved() {
    loadData();
  }

  onMount(() => {
    loadData();
  });

  const filteredData = $derived.by(() => {
    let result = Object.entries(groupedData).sort((a, b) => b[0].localeCompare(a[0]));
    
    if (searchQuery) {
      result = result.map(([date, data]) => [
        date,
        {
          ...data,
          slots: data.slots.filter(slot =>
            slotTitle(slot).toLowerCase().includes(searchQuery.toLowerCase())
          ),
        },
      ] as [string, typeof data]).filter(([, data]) => data.slots.length > 0);
    }

    if (filterType) {
      result = result.map(([date, data]) => [
        date,
        {
          ...data,
          slots: data.slots.filter(slot => slot.slot_type === filterType),
        },
      ] as [string, typeof data]).filter(([, data]) => data.slots.length > 0);
    }

    return result;
  });

  const totalStats = $derived.by(() => {
    let totalMinutes = 0;
    let totalCount = 0;
    for (const [, data] of Object.entries(groupedData)) {
      totalMinutes += data.stats.total_recorded;
      totalCount += data.slots.length;
    }
    return { totalMinutes, totalCount };
  });
</script>

<div class="flex h-screen">
  <!-- Main Content -->
  <div class="flex-1 flex flex-col">
    <!-- Toolbar -->
    <div class="flex items-center justify-between border-b border-border px-6 py-4">
      <div class="flex items-center gap-3">
        <!-- View toggle -->
        <div class="flex rounded-md border border-border">
          <button
            onclick={() => viewMode = 'list'}
            class="flex items-center gap-2 rounded-l-md px-3 py-2 text-sm font-medium transition-colors {viewMode === 'list'
              ? 'bg-accent text-accent-foreground'
              : 'text-muted-foreground hover:bg-accent/50'}"
          >
            <List size={16} />
            {$t('records.list_view')}
          </button>
          <button
            onclick={() => viewMode = 'table'}
            class="flex items-center gap-2 rounded-r-md px-3 py-2 text-sm font-medium transition-colors {viewMode === 'table'
              ? 'bg-accent text-accent-foreground'
              : 'text-muted-foreground hover:bg-accent/50'}"
          >
            <TableIcon size={16} />
            {$t('records.table_view')}
          </button>
        </div>

        <!-- Date range -->
        <button
          onclick={() => showDatePicker = !showDatePicker}
          class="flex items-center gap-2 rounded-md border border-border px-3 py-2 text-sm font-medium hover:bg-accent"
        >
          <Calendar size={16} />
          {formatDate(startDate)} - {formatDate(endDate)}
        </button>

        <!-- Filter -->
        <button
          onclick={() => showFilterDialog = !showFilterDialog}
          class="flex items-center gap-2 rounded-md border border-border px-3 py-2 text-sm font-medium hover:bg-accent {filterType ? 'bg-accent' : ''}"
        >
          <Filter size={16} />
          {filterType ? $t(`slot_types.${filterType}`) : $t('records.filter')}
        </button>

        <!-- Search -->
        <input
          type="search"
          bind:value={searchQuery}
          placeholder={$t('records.search_placeholder')}
          class="w-64 rounded-md border border-border bg-background px-3 py-2 text-sm placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
        />
      </div>

      <!-- Export -->
      <button
        onclick={() => showExportDialog = true}
        class="flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
      >
        <Download size={16} />
        {$t('records.export')}
      </button>
    </div>

    <!-- Records Content -->
    <div class="flex-1 overflow-y-auto">
      {#if loading}
        <div class="flex items-center justify-center h-64">
          <div class="text-muted-foreground">{$t('common.loading')}</div>
        </div>
      {:else if filteredData.length === 0}
        <div class="flex items-center justify-center h-64">
          <div class="text-muted-foreground">{$t('records.no_records')}</div>
        </div>
      {:else}
        <div class="divide-y divide-border">
          {#each filteredData as [date, data] (date)}
            <!-- Day group header -->
            <div class="bg-muted/30 px-6 py-2 text-sm font-medium sticky top-0">
              <div class="flex items-center justify-between">
                <span>{formatDate(date)}</span>
                <div class="flex items-center gap-4 text-muted-foreground">
                  <span>{formatDuration(data.stats.total_recorded)} / 8h</span>
                  <span>{data.slots.length} {$t('records.items')}</span>
                </div>
              </div>
            </div>

            <!-- Slot items -->
            {#each data.slots as slot (slot.id)}
              {@const end = slotEnd(slot)}
              {@const Icon = slotTypeIcons[slot.slot_type]}
              <button
                onclick={() => handleEditSlot(slot)}
                class="flex w-full items-center gap-3 px-6 py-3 text-left hover:bg-accent/50 transition-colors {slot.parent_id ? 'pl-12' : ''}"
              >
                <div class="flex size-10 flex-shrink-0 items-center justify-center rounded-md bg-muted">
                  <Icon size={20} class="text-muted-foreground" />
                </div>
                <div class="flex-1 min-w-0">
                  <div class="flex items-center gap-2">
                    <div class="font-medium truncate">{slotTitle(slot)}</div>
                    {#if slot.parent_id}
                      <span class="flex-shrink-0 rounded bg-accent px-2 py-0.5 text-xs font-medium text-accent-foreground">
                        {$t('timeline.interrupt_badge')}
                      </span>
                    {/if}
                    {#if slot.person}
                      <span class="flex-shrink-0 text-xs text-muted-foreground">@{slot.person}</span>
                    {/if}
                  </div>
                  <div class="mt-1 text-sm text-muted-foreground">
                    {formatTime(slotStart(slot))}-{end ? formatTime(end) : $t('timer.ongoing')} · {slot.duration_minutes || 0} {$t('time.minute_short')}
                  </div>
                </div>
              </button>
            {/each}
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <!-- Summary Side Panel -->
  <div class="w-80 border-l border-border bg-card flex-shrink-0">
    <div class="p-6">
      <h3 class="mb-4 text-sm font-semibold">{$t('records.summary_by_task')}</h3>
      <div class="space-y-3">
        {#each taskSummary.slice(0, 10) as item (`${item.task}-${item.minutes}`)}
          <div class="flex items-center justify-between">
            <div class="min-w-0 flex-1">
              <div class="truncate font-medium">{item.task}</div>
              <div class="h-2 mt-1.5 overflow-hidden rounded-full bg-muted">
                <div
                  class="h-full bg-primary"
                  style="width: {Math.min((item.minutes / totalStats.totalMinutes) * 100, 100)}%"
                ></div>
              </div>
            </div>
            <div class="ml-4 flex-shrink-0 text-sm font-semibold tabular-nums">
              {formatDuration(item.minutes)}
            </div>
          </div>
        {/each}
      </div>

      <div class="mt-6 pt-6 border-t border-border">
        <div class="flex items-center justify-between text-sm">
          <span class="text-muted-foreground">
            {$t('records.total_items', { count: totalStats.totalCount })}
          </span>
          <span class="font-medium">{formatDuration(totalStats.totalMinutes)}</span>
        </div>
      </div>
    </div>
  </div>
</div>

<SlotEditDrawer bind:open={showEditDrawer} bind:slot={editingSlot} onSaved={handleSlotSaved} />
