<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { save } from '@tauri-apps/plugin-dialog';
  import { List, Table as TableIcon, Filter, Download, Calendar, Briefcase, Zap, Users, Coffee, Clock, AlertTriangle, Check } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import type { Category, CategoryReport, TimeSlot, DayStats } from '$lib/types';
  import SlotEditDrawer from './SlotEditDrawer.svelte';
  import EodGapReviewDialog from './EodGapReviewDialog.svelte';

  type ViewMode = 'list' | 'table';
  type GroupedSlots = Record<string, { slots: TimeSlot[]; stats: DayStats }>;

  let viewMode = $state<ViewMode>('list');
  let startDate = $state(getDateString(new Date(Date.now() - 7 * 24 * 60 * 60 * 1000)));
  let endDate = $state(getDateString(new Date()));
  let searchQuery = $state('');
  let filterType = $state<string | null>(null);
  let filterCategory = $state<number | 'none' | null>(null);
  let categories = $state<Category[]>([]);
  let summaryMode = $state<'task' | 'category'>('task');
  let categoryReport = $state<CategoryReport | null>(null);

  let groupedData = $state<GroupedSlots>({});
  let taskSummary = $state<Array<{ task: string; minutes: number }>>([]);
  let loading = $state(true);

  let editingSlot = $state<TimeSlot | null>(null);
  let showEditDrawer = $state(false);
  let showFilterDialog = $state(false);
  let showExportDialog = $state(false);
  let showDatePicker = $state(false);
  let includeInterrupts = $state(true);
  let exporting = $state(false);
  let exportKind = $state<'csv' | 'xlsx' | null>(null);
  let exportError = $state('');
  let exportMessage = $state('');
  let exportCount = $state<number | null>(null);
  let exportCancelled = $state(false);
  let selectedIds = $state<number[]>([]);
  let mergeError = $state('');
  let merging = $state(false);

  const slotTypeIcons = {
    work: Briefcase,
    interrupt: Zap,
    meeting: Users,
    lunch: Coffee,
  };

  function localDateString(date: Date): string {
    const month = String(date.getMonth() + 1).padStart(2, '0');
    const day = String(date.getDate()).padStart(2, '0');
    return `${date.getFullYear()}-${month}-${day}`;
  }

  function getDateString(date: Date): string {
    return localDateString(date);
  }

  function formatDate(isoDate: string): string {
    const [year, month, day] = isoDate.split('-').map(Number);
    const date = new Date(year, (month || 1) - 1, day || 1);
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

  function slotDate(slot: TimeSlot): string {
    const raw = slotStart(slot);
    if (!raw) return '';
    const parsed = new Date(raw);
    if (Number.isNaN(parsed.getTime())) return raw.split('T')[0];
    return localDateString(parsed);
  }

  function slotMinutes(slot: TimeSlot): number {
    const start = Date.parse(slotStart(slot));
    if (Number.isNaN(start)) return 0;
    const endRaw = slotEnd(slot);
    const end = endRaw ? Date.parse(endRaw) : Date.now();
    if (Number.isNaN(end)) return 0;
    return Math.max(0, Math.round((end - start) / 60000));
  }

  function summarize(slots: TimeSlot[]): DayStats {
    const stats: DayStats = {
      work_minutes: 0,
      interrupt_minutes: 0,
      meeting_minutes: 0,
      lunch_minutes: 0,
      total_recorded: 0,
    };
    for (const slot of slots) {
      const minutes = slotMinutes(slot);
      if (slot.slot_type === 'interrupt') stats.interrupt_minutes += minutes;
      else if (slot.slot_type === 'meeting') stats.meeting_minutes += minutes;
      else if (slot.slot_type === 'lunch') stats.lunch_minutes += minutes;
      else stats.work_minutes += minutes;
      stats.total_recorded += minutes;
    }
    return stats;
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
      const request = { start_date: startDate, end_date: endDate };
      const [slots, report, categoryList] = await Promise.all([
        invoke<TimeSlot[]>('get_slots_for_range', { request }),
        invoke<CategoryReport>('get_category_report', { request }),
        invoke<Category[]>('get_categories'),
      ]);
      categoryReport = report;
      categories = categoryList;

      const grouped: GroupedSlots = {};
      for (const slot of slots) {
        const date = slotDate(slot);
        if (!date) continue;
        if (!grouped[date]) {
          grouped[date] = {
            slots: [],
            stats: {
              work_minutes: 0,
              interrupt_minutes: 0,
              meeting_minutes: 0,
              lunch_minutes: 0,
              total_recorded: 0,
            },
          };
        }
        grouped[date].slots.push(slot);
      }

      for (const data of Object.values(grouped)) {
        data.stats = summarize(data.slots);
      }
      groupedData = grouped;

      const totals: Record<string, number> = {};
      for (const slot of slots) {
        const name = slotTitle(slot) || '未命名任務';
        totals[name] = (totals[name] ?? 0) + slotMinutes(slot);
      }
      taskSummary = Object.entries(totals)
        .map(([task, minutes]) => ({ task, minutes }))
        .sort((a, b) => b.minutes - a.minutes);
    } catch (err) {
      console.error('Failed to load records:', err);
    } finally {
      loading = false;
    }
  }

  function matchesCategory(slot: TimeSlot): boolean {
    if (filterCategory === 'none') {
      return slot.slot_type !== 'lunch' && slot.category_id == null;
    }
    return slot.category_id === filterCategory;
  }

  function handleEditSlot(slot: TimeSlot) {
    editingSlot = slot;
    showEditDrawer = true;
  }

  function handleSlotSaved() {
    loadData();
  }

  function toggleSelect(id: number) {
    selectedIds = selectedIds.includes(id)
      ? selectedIds.filter((item) => item !== id)
      : [...selectedIds, id];
    mergeError = '';
  }

  async function mergeSelected() {
    if (selectedIds.length < 2) {
      mergeError = $t('errors.merge_need_two');
      return;
    }
    merging = true;
    mergeError = '';
    try {
      await invoke('merge_slots', { request: { ids: selectedIds, task_name: null } });
      selectedIds = [];
      await loadData();
    } catch (err) {
      mergeError = String(err);
    } finally {
      merging = false;
    }
  }

  function openExport() {
    exportError = '';
    exportMessage = '';
    exportCount = null;
    exportCancelled = false;
    showExportDialog = true;
  }

  function currentTypeLabels() {
    return {
      work: $t('slot_types.work'),
      interrupt: $t('slot_types.interrupt'),
      meeting: $t('slot_types.meeting'),
      lunch: $t('slot_types.lunch'),
      uncategorized: $t('categories.uncategorized'),
    };
  }

  async function exportFile(kind: 'xlsx' | 'csv') {
    exportError = '';
    exportMessage = '';
    exportCount = null;
    exportCancelled = false;
    exportKind = kind;
    const extension = kind === 'xlsx' ? 'xlsx' : 'csv';
    exporting = true;
    try {
      const path = await save({
        defaultPath: `工時記錄_${startDate}_${endDate}.${extension}`,
        filters: [{ name: kind === 'xlsx' ? 'Excel' : 'CSV', extensions: [extension] }],
      });
      if (!path) {
        exportCancelled = true;
        return;
      }
      const saved = await invoke<{ path: string; count: number }>(
        kind === 'xlsx' ? 'export_to_xlsx' : 'export_to_csv',
        {
          request: {
            start_date: startDate,
            end_date: endDate,
            include_interrupts: includeInterrupts,
            type_labels: currentTypeLabels(),
          },
          outputPath: path,
        },
      );
      exportMessage = saved.path;
      exportCount = saved.count;
    } catch (err) {
      exportError = String(err);
    } finally {
      exporting = false;
      exportKind = null;
    }
  }

  onMount(() => {
    loadData();
    const refresh = () => loadData();
    window.addEventListener('task-started', refresh);
    return () => window.removeEventListener('task-started', refresh);
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

    if (filterCategory !== null) {
      result = result.map(([date, data]) => [
        date,
        {
          ...data,
          slots: data.slots.filter(matchesCategory),
        },
      ] as [string, typeof data]).filter(([, data]) => data.slots.length > 0);
    }

    // Day totals follow what is on screen.
    return result.map(([date, data]) => [date, { ...data, stats: summarize(data.slots) }] as [string, typeof data]);
  });

  const activeFilterLabel = $derived.by(() => {
    const parts: string[] = [];
    if (filterType) parts.push($t(`slot_types.${filterType}`));
    if (filterCategory === 'none') parts.push($t('categories.uncategorized'));
    else if (filterCategory !== null) {
      const match = categories.find((category) => category.id === filterCategory);
      if (match) parts.push(match.name);
    }
    return parts.join(' · ');
  });

  const totalStats = $derived.by(() => {
    const seen: Record<string, true> = {};
    let totalMinutes = 0;
    let totalCount = 0;
    for (const [, data] of Object.entries(groupedData)) {
      for (const slot of data.slots) {
        if (slot.id == null) continue;
        const key = String(slot.id);
        if (seen[key]) continue;
        seen[key] = true;
        totalMinutes += slotMinutes(slot);
        totalCount += 1;
      }
    }
    return { totalMinutes, totalCount };
  });
</script>

<div class="flex h-screen">
  <!-- Main Content -->
  <div class="flex-1 flex flex-col">
    <!-- Toolbar -->
    <div class="flex flex-wrap items-center justify-between gap-3 border-b border-border px-6 py-4">
      <div class="flex min-w-0 flex-wrap items-center gap-3">
        <!-- View toggle -->
        <div class="flex shrink-0 rounded-md border border-border">
          <button
            onclick={() => viewMode = 'list'}
            class="flex shrink-0 items-center gap-2 whitespace-nowrap rounded-l-md px-3 py-2 text-sm font-medium transition-colors {viewMode === 'list'
              ? 'bg-accent text-accent-foreground'
              : 'text-muted-foreground hover:bg-accent/50'}"
          >
            <List size={16} class="shrink-0" />
            {$t('records.list_view')}
          </button>
          <button
            onclick={() => viewMode = 'table'}
            class="flex shrink-0 items-center gap-2 whitespace-nowrap rounded-r-md px-3 py-2 text-sm font-medium transition-colors {viewMode === 'table'
              ? 'bg-accent text-accent-foreground'
              : 'text-muted-foreground hover:bg-accent/50'}"
          >
            <TableIcon size={16} class="shrink-0" />
            {$t('records.table_view')}
          </button>
        </div>

        <!-- Date range -->
        <button
          onclick={() => showDatePicker = !showDatePicker}
          class="flex shrink-0 items-center gap-2 whitespace-nowrap rounded-md border border-border px-3 py-2 text-sm font-medium hover:bg-accent"
        >
          <Calendar size={16} class="shrink-0" />
          {formatDate(startDate)} - {formatDate(endDate)}
        </button>

        <!-- Filter -->
        <div class="relative shrink-0">
          <button
            onclick={() => showFilterDialog = !showFilterDialog}
            aria-expanded={showFilterDialog}
            class="flex shrink-0 items-center gap-2 whitespace-nowrap rounded-md border border-border px-3 py-2 text-sm font-medium hover:bg-accent {activeFilterLabel ? 'bg-accent' : ''}"
          >
            <Filter size={16} class="shrink-0" />
            {activeFilterLabel || $t('records.filter')}
          </button>
          {#if showFilterDialog}
            <div class="absolute left-0 top-full z-20 mt-2 w-64 rounded-md border border-border bg-popover p-3 text-popover-foreground shadow-lg">
              <div class="mb-1.5 text-xs font-medium text-muted-foreground">{$t('categories.status')}</div>
              <div class="flex flex-wrap gap-1.5">
                {#each [null, 'work', 'meeting', 'interrupt', 'lunch'] as type (type)}
                  <button
                    onclick={() => filterType = type}
                    aria-pressed={filterType === type}
                    class="whitespace-nowrap rounded border border-border px-2 py-1 text-xs {filterType === type ? 'bg-primary text-primary-foreground' : 'hover:bg-accent'}"
                  >
                    {type ? $t(`slot_types.${type}`) : $t('categories.all_statuses')}
                  </button>
                {/each}
              </div>
              <div class="mb-1.5 mt-3 text-xs font-medium text-muted-foreground">{$t('categories.label')}</div>
              <div class="flex flex-wrap gap-1.5">
                <button
                  onclick={() => filterCategory = null}
                  aria-pressed={filterCategory === null}
                  class="whitespace-nowrap rounded border border-border px-2 py-1 text-xs {filterCategory === null ? 'bg-primary text-primary-foreground' : 'hover:bg-accent'}"
                >
                  {$t('categories.all_categories')}
                </button>
                <button
                  onclick={() => filterCategory = 'none'}
                  aria-pressed={filterCategory === 'none'}
                  class="whitespace-nowrap rounded border border-border px-2 py-1 text-xs {filterCategory === 'none' ? 'bg-primary text-primary-foreground' : 'hover:bg-accent'}"
                >
                  {$t('categories.uncategorized')}
                </button>
                {#each categories as category (category.id)}
                  <button
                    onclick={() => filterCategory = category.id}
                    aria-pressed={filterCategory === category.id}
                    class="whitespace-nowrap rounded border border-border px-2 py-1 text-xs {filterCategory === category.id ? 'bg-primary text-primary-foreground' : 'hover:bg-accent'} {category.hidden ? 'opacity-60' : ''}"
                  >
                    {category.name}
                  </button>
                {/each}
              </div>
              {#if filterType || filterCategory !== null}
                <button
                  onclick={() => { filterType = null; filterCategory = null; }}
                  class="mt-3 whitespace-nowrap text-xs font-medium text-muted-foreground hover:text-foreground"
                >
                  {$t('categories.clear_filter')}
                </button>
              {/if}
            </div>
          {/if}
        </div>

        <!-- Search -->
        <input
          type="search"
          bind:value={searchQuery}
          placeholder={$t('records.search_placeholder')}
          class="min-w-40 max-w-64 flex-1 rounded-md border border-border bg-background px-3 py-2 text-sm placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
        />
      </div>

      <!-- Export -->
      <button
        onclick={openExport}
        class="flex shrink-0 items-center gap-2 whitespace-nowrap rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
      >
        <Download size={16} class="shrink-0" />
        {$t('records.export')}
      </button>
    </div>

    {#if selectedIds.length > 0}
      <div class="flex items-center justify-between gap-3 border-b border-border px-6 py-2 text-sm">
        <span class="text-muted-foreground">{$t('today.selected', { count: selectedIds.length })}</span>
        <div class="flex items-center gap-2">
          {#if mergeError}
            <span class="max-w-xs truncate text-destructive">{mergeError}</span>
          {/if}
          <button
            onclick={() => { selectedIds = []; mergeError = ''; }}
            class="rounded-md px-3 py-1.5 font-medium text-muted-foreground hover:bg-muted"
          >
            {$t('actions.cancel')}
          </button>
          <button
            onclick={mergeSelected}
            disabled={merging || selectedIds.length < 2}
            class="rounded-md bg-primary px-3 py-1.5 font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          >
            {$t('actions.merge')}
          </button>
        </div>
      </div>
    {/if}

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
            <div class="sticky top-0 z-10 border-b border-border bg-muted px-6 py-2 text-sm font-medium">
              <div class="flex items-center justify-between gap-4">
                <span class="shrink-0 whitespace-nowrap">{formatDate(date)}</span>
                <div class="flex shrink-0 items-center gap-4 text-muted-foreground">
                  <span class="whitespace-nowrap tabular-nums">{formatDuration(data.stats.total_recorded)} / 8h</span>
                  <span class="whitespace-nowrap">{data.slots.length} {$t('records.items')}</span>
                </div>
              </div>
            </div>

            <!-- Slot items -->
            {#each data.slots as slot (slot.id)}
              {@const end = slotEnd(slot)}
              {@const Icon = slotTypeIcons[slot.slot_type]}
              <div class="flex items-stretch {slot.parent_id ? 'pl-6' : ''}">
              <label class="flex items-center pl-4">
                <input
                  type="checkbox"
                  checked={selectedIds.includes(Number(slot.id))}
                  onchange={() => toggleSelect(Number(slot.id))}
                  aria-label={slotTitle(slot)}
                  class="size-4 rounded border-input"
                />
              </label>
              <button
                onclick={() => handleEditSlot(slot)}
                class="flex w-full items-center gap-3 px-4 py-3 text-left hover:bg-accent/50 transition-colors"
              >
                <div class="flex size-10 flex-shrink-0 items-center justify-center rounded-md bg-muted">
                  <Icon size={20} class="text-muted-foreground" />
                </div>
                <div class="flex-1 min-w-0">
                  <div class="flex items-center gap-2">
                    <div class="font-medium truncate">{slotTitle(slot)}</div>
                    {#if slot.slot_type !== 'lunch'}
                      <span class="flex-shrink-0 whitespace-nowrap rounded border border-border px-2 py-0.5 text-xs text-muted-foreground">
                        {slot.category_name ?? $t('categories.uncategorized')}
                      </span>
                    {/if}
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
                    {formatTime(slotStart(slot))}-{end ? formatTime(end) : $t('today.ongoing')} · {slotMinutes(slot)} {$t('time.minute_short')}
                  </div>
                </div>
              </button>
              </div>
            {/each}
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <!-- Summary Side Panel -->
  <div class="w-80 border-l border-border bg-card flex-shrink-0">
    <div class="p-6">
      <div class="mb-4 flex rounded-md border border-border" role="group">
        <button
          onclick={() => summaryMode = 'task'}
          aria-pressed={summaryMode === 'task'}
          class="flex-1 whitespace-nowrap rounded-l-md px-2 py-1.5 text-sm font-medium transition-colors {summaryMode === 'task' ? 'bg-accent text-accent-foreground' : 'text-muted-foreground hover:bg-accent/50'}"
        >
          {$t('records.summary_by_task')}
        </button>
        <button
          onclick={() => summaryMode = 'category'}
          aria-pressed={summaryMode === 'category'}
          class="flex-1 whitespace-nowrap rounded-r-md px-2 py-1.5 text-sm font-medium transition-colors {summaryMode === 'category' ? 'bg-accent text-accent-foreground' : 'text-muted-foreground hover:bg-accent/50'}"
        >
          {$t('categories.summary_by_category')}
        </button>
      </div>
      {#if summaryMode === 'task'}
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
      {:else}
        <div class="space-y-3">
          {#each categoryReport?.rows ?? [] as row (row.category_id ?? 'none')}
            <div class="flex items-center justify-between">
              <div class="min-w-0 flex-1">
                <div class="truncate font-medium">{row.name ?? $t('categories.uncategorized')}</div>
                <div class="h-2 mt-1.5 overflow-hidden rounded-full bg-muted">
                  <div class="h-full bg-primary" style="width: {row.percentage}%"></div>
                </div>
              </div>
              <div class="ml-4 flex-shrink-0 text-sm font-semibold tabular-nums">
                {formatDuration(Math.round(row.total_seconds / 60))}
              </div>
            </div>
          {/each}
        </div>
        <p class="mt-3 text-xs text-muted-foreground">{$t('report.excludes_lunch')}</p>
      {/if}

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

{#if showEditDrawer && editingSlot}
  {#key editingSlot.id}
    <SlotEditDrawer bind:open={showEditDrawer} bind:slot={editingSlot} onSaved={handleSlotSaved} />
  {/key}
{/if}

{#if showExportDialog}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
    <button
      type="button"
      class="absolute inset-0 bg-black/40"
      aria-label={$t('common.cancel')}
      onclick={() => { showExportDialog = false; }}
    ></button>
    <div
      role="dialog"
      aria-labelledby="export-title"
      class="relative z-10 w-full max-w-md rounded-lg border border-border bg-card p-6 text-card-foreground shadow-lg"
    >
      <h2 id="export-title" class="text-lg font-semibold">{$t('records.export_dialog_title')}</h2>
      <p class="mt-2 text-sm text-muted-foreground">{formatDate(startDate)} - {formatDate(endDate)}</p>
      <label class="mt-4 flex items-center gap-2 text-sm">
        <input type="checkbox" bind:checked={includeInterrupts} />
        {$t('records.include_interrupts')}
      </label>
      <div class="mt-4" aria-live="polite">
        {#if exporting}
          <p class="text-sm font-medium text-foreground">{$t('records.exporting')}</p>
        {:else if exportError}
          <p class="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">{exportError}</p>
        {:else if exportCancelled}
          <p class="rounded-md border border-border bg-muted px-3 py-2 text-sm text-foreground">{$t('records.export_cancelled')}</p>
        {:else if exportCount !== null}
          <div class="rounded-md border border-primary/30 bg-primary/10 px-3 py-3" role="status">
            <div class="flex items-center gap-2 font-medium text-foreground">
              <Check class="size-4" />
              {#if exportCount === 0}
                {$t('records.export_empty')}
              {:else}
                {$t('records.export_success', { count: exportCount })}
              {/if}
            </div>
            <p class="mt-2 text-xs text-muted-foreground">{$t('records.export_saved_to')}</p>
            <p class="mt-1 break-all text-sm text-foreground">{exportMessage}</p>
          </div>
        {/if}
      </div>
      <div class="mt-6 flex justify-end gap-2">
        <button
          type="button"
          onclick={() => { showExportDialog = false; }}
          disabled={exporting}
          class="rounded-md px-3 py-2 text-sm font-medium text-muted-foreground hover:bg-muted disabled:opacity-50"
        >
          {exportCount !== null ? $t('common.close') : $t('common.cancel')}
        </button>
        <button
          type="button"
          onclick={() => exportFile('csv')}
          disabled={exporting}
          class="rounded-md border border-border px-3 py-2 text-sm font-medium hover:bg-accent disabled:opacity-50"
        >
          {exporting && exportKind === 'csv' ? $t('records.exporting') : 'CSV'}
        </button>
        <button
          type="button"
          onclick={() => exportFile('xlsx')}
          disabled={exporting}
          class="rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        >
          {exporting && exportKind === 'xlsx' ? $t('records.exporting') : 'Excel'}
        </button>
      </div>
    </div>
  </div>
{/if}
