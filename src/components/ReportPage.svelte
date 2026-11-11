<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { save } from '@tauri-apps/plugin-dialog';
  import { Copy, Download } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import {
    buildSeries,
    DARK_CHART,
    formatHours,
    formatPercent,
    imageToPng,
    LIGHT_CHART,
    loadReportBundle,
    pieChartSvg,
    presetRange,
    reportImage,
    stackedBarSvg,
    summaryParts,
    type ReportPreset,
  } from '$lib/report';
  import type { Category, CategoryReport } from '$lib/types';

  type ChartKind = 'pie' | 'bar';

  const presets: ReportPreset[] = ['this_week', 'last_week', 'this_month', 'custom'];

  const initial = presetRange('this_week', new Date());

  let preset = $state<ReportPreset>('this_week');
  let startDate = $state(initial.start);
  let endDate = $state(initial.end);
  let chartKind = $state<ChartKind>('pie');
  let report = $state<CategoryReport | null>(null);
  let categories = $state<Category[]>([]);
  let loadError = $state('');
  let isDark = $state(false);
  let notice = $state('');
  let noticeIsError = $state(false);
  let busy = $state(false);

  const invalidRange = $derived(endDate < startDate);
  const theme = $derived(isDark ? DARK_CHART : LIGHT_CHART);
  const series = $derived(
    report
      ? buildSeries(report, categories, theme, {
          uncategorized: $t('categories.uncategorized'),
        })
      : [],
  );
  const hasInterrupts = $derived(series.some((item) => item.interruptSeconds > 0));
  const chartSvg = $derived(
    !report || report.total_seconds === 0
      ? ''
      : chartKind === 'pie'
        ? pieChartSvg(series, theme)
        : stackedBarSvg(report, series, theme, $t('report.hours')),
  );
  const rangeLabel = $derived(
    preset === 'custom'
      ? `${startDate.slice(5).replace('-', '/')} – ${endDate.slice(5).replace('-', '/')}`
      : $t(`time.${preset}`),
  );
  const summary = $derived(
    report && report.total_seconds > 0
      ? $t('report.summary', {
          range: rangeLabel,
          hours: formatHours(report.total_seconds),
          parts: summaryParts(
            report,
            categories,
            $t('categories.uncategorized'),
            $t('report.list_separator'),
          ),
        })
      : '',
  );
  const footnote = $derived(
    [$t('report.excludes_lunch'), hasInterrupts ? $t('report.interrupt_note') : '']
      .filter(Boolean)
      .join(' · '),
  );

  // Names and colours come from the category list read here, so a new or
  // renamed category shows up when the report opens and when the range changes.
  async function load() {
    if (invalidRange) return;
    loadError = '';
    try {
      const loaded = await loadReportBundle(
        {
          getCategoryReport: (start, end) =>
            invoke<CategoryReport>('get_category_report', {
              request: { start_date: start, end_date: end },
            }),
          getCategories: () => invoke<Category[]>('get_categories'),
        },
        startDate,
        endDate,
      );
      report = loaded.report;
      categories = loaded.categories;
    } catch (err) {
      loadError = String(err);
    }
  }

  function choosePreset(next: ReportPreset) {
    preset = next;
    notice = '';
    if (next === 'custom') return;
    const range = presetRange(next, new Date());
    startDate = range.start;
    endDate = range.end;
    load();
  }

  function buildImage() {
    return reportImage(chartSvg, chartKind === 'pie' ? 1 : 300 / 640, series, theme, {
      title: $t('report.title'),
      summary,
      note: footnote,
    });
  }

  async function exportPng() {
    notice = '';
    busy = true;
    try {
      const path = await save({
        defaultPath: `${$t('report.title')}_${startDate}_${endDate}.png`,
        filters: [{ name: 'PNG', extensions: ['png'] }],
      });
      if (!path) return;
      const blob = await imageToPng(buildImage());
      const bytes = Array.from(new Uint8Array(await blob.arrayBuffer()));
      const saved = await invoke<string>('save_report_image', { outputPath: path, bytes });
      noticeIsError = false;
      notice = $t('report.saved', { path: saved });
    } catch (err) {
      noticeIsError = true;
      notice = $t('report.export_failed', { error: String(err) });
    } finally {
      busy = false;
    }
  }

  async function copyImage() {
    notice = '';
    busy = true;
    try {
      // The blob stays a promise so the write still counts as part of the click.
      const item = new ClipboardItem({ 'image/png': imageToPng(buildImage()) });
      await navigator.clipboard.write([item]);
      noticeIsError = false;
      notice = $t('report.copied');
    } catch (err) {
      console.error('Failed to copy report image:', err);
      noticeIsError = true;
      notice = $t('report.copy_failed');
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    const root = document.documentElement;
    const syncTheme = () => {
      isDark = root.classList.contains('dark');
    };
    syncTheme();
    const observer = new MutationObserver(syncTheme);
    observer.observe(root, { attributes: true, attributeFilter: ['class'] });
    load();
    return () => observer.disconnect();
  });
</script>

<div class="flex h-screen flex-col">
  <!-- Toolbar -->
  <div class="flex flex-wrap items-center justify-between gap-3 border-b border-border px-6 py-4">
    <div class="flex min-w-0 flex-wrap items-center gap-3">
      <div class="flex shrink-0 rounded-md border border-border" role="group" aria-label={$t('records.date_range')}>
        {#each presets as item (item)}
          <button
            onclick={() => choosePreset(item)}
            aria-pressed={preset === item}
            class="whitespace-nowrap px-3 py-2 text-sm font-medium transition-colors first:rounded-l-md last:rounded-r-md {preset === item
              ? 'bg-accent text-accent-foreground'
              : 'text-muted-foreground hover:bg-accent/50'}"
          >
            {item === 'custom' ? $t('report.custom') : $t(`time.${item}`)}
          </button>
        {/each}
      </div>

      {#if preset === 'custom'}
        <div class="flex shrink-0 items-center gap-2">
          <input
            type="date"
            bind:value={startDate}
            onchange={load}
            aria-label={$t('report.start_date')}
            class="rounded-md border border-border bg-background px-3 py-1.5 text-sm"
          />
          <span class="text-muted-foreground">–</span>
          <input
            type="date"
            bind:value={endDate}
            onchange={load}
            aria-label={$t('report.end_date')}
            class="rounded-md border border-border bg-background px-3 py-1.5 text-sm"
          />
        </div>
      {/if}

      <div class="flex shrink-0 rounded-md border border-border" role="group">
        {#each ['pie', 'bar'] as const as kind (kind)}
          <button
            onclick={() => (chartKind = kind)}
            aria-pressed={chartKind === kind}
            class="whitespace-nowrap px-3 py-2 text-sm font-medium transition-colors first:rounded-l-md last:rounded-r-md {chartKind === kind
              ? 'bg-accent text-accent-foreground'
              : 'text-muted-foreground hover:bg-accent/50'}"
          >
            {$t(`report.${kind}`)}
          </button>
        {/each}
      </div>
    </div>

    <div class="flex shrink-0 items-center gap-2">
      <button
        onclick={copyImage}
        disabled={busy || !chartSvg}
        class="flex shrink-0 items-center gap-2 whitespace-nowrap rounded-md border border-border px-3 py-2 text-sm font-medium hover:bg-accent disabled:opacity-50"
      >
        <Copy size={16} class="shrink-0" />
        {$t('report.copy_image')}
      </button>
      <button
        onclick={exportPng}
        disabled={busy || !chartSvg}
        class="flex shrink-0 items-center gap-2 whitespace-nowrap rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
      >
        <Download size={16} class="shrink-0" />
        {$t('report.export_png')}
      </button>
    </div>
  </div>

  <div class="flex-1 overflow-y-auto p-6">
    {#if notice}
      <p
        class="mb-4 break-all rounded-md border px-3 py-2 text-sm {noticeIsError
          ? 'border-destructive/30 bg-destructive/10 text-destructive'
          : 'border-border bg-muted text-foreground'}"
        role="status"
      >
        {notice}
      </p>
    {/if}

    {#if invalidRange}
      <p class="text-sm text-destructive">{$t('report.invalid_range')}</p>
    {:else if loadError}
      <p class="text-sm text-destructive">{loadError}</p>
    {:else if !report}
      <div class="text-muted-foreground">{$t('common.loading')}</div>
    {:else if report.total_seconds === 0}
      <div class="flex h-64 items-center justify-center text-muted-foreground">
        {$t('report.empty')}
      </div>
    {:else}
      <p class="mb-6 text-base font-medium">{summary}</p>

      <div class="grid gap-8 lg:grid-cols-[minmax(0,3fr)_minmax(16rem,2fr)]">
        <div class="mx-auto w-full {chartKind === 'pie' ? 'max-w-sm' : ''}">
          <!-- report.ts escapes every name it writes into the markup. -->
          {@html chartSvg}
        </div>

        <ul class="space-y-2 self-center">
          {#each series as item (item.key)}
            <li class="flex items-center gap-2 text-sm">
              {@render swatch(item, 'legend')}
              <span class="min-w-0 flex-1 truncate">{item.name}</span>
              <span class="shrink-0 tabular-nums text-muted-foreground">
                {formatHours(item.seconds)}h · {formatPercent(item.percentage)}
              </span>
            </li>
          {/each}
        </ul>
      </div>

      <table class="mt-8 w-full text-sm">
        <thead>
          <tr class="border-b border-border text-left text-muted-foreground">
            <th class="py-2 font-medium">{$t('report.category')}</th>
            <th class="py-2 text-right font-medium">{$t('report.hours')}</th>
            <th class="py-2 text-right font-medium">{$t('report.share')}</th>
          </tr>
        </thead>
        <tbody>
          {#each report.rows as row (row.category_id ?? 'none')}
            {@const item = series.find((entry) => entry.key === (row.category_id == null ? 'none' : `c${row.category_id}`))}
            <tr class="border-b border-border">
              <td class="py-2">
                <span class="inline-flex items-center gap-2">
                  {#if item}
                    {@render swatch(item, 'table')}
                  {/if}
                  {item?.name ?? row.name ?? $t('categories.uncategorized')}
                </span>
              </td>
              <td class="py-2 text-right tabular-nums">{formatHours(row.total_seconds)}</td>
              <td class="py-2 text-right tabular-nums">{formatPercent(row.percentage)}</td>
            </tr>
          {/each}
        </tbody>
        <tfoot>
          <tr class="font-semibold">
            <td class="py-2">{$t('report.total')}</td>
            <td class="py-2 text-right tabular-nums">{formatHours(report.total_seconds)}</td>
            <td class="py-2 text-right tabular-nums">100%</td>
          </tr>
        </tfoot>
      </table>

      <p class="mt-4 text-xs text-muted-foreground">{footnote}</p>
    {/if}
  </div>
</div>

{#snippet swatch(item, salt)}
  <svg width="12" height="12" viewBox="0 0 12 12" class="size-3 shrink-0" aria-hidden="true">
    {#if item.pattern != null}
      <defs>
        <pattern
          id="swatch-{salt}-{item.key}"
          width="4"
          height="4"
          patternUnits="userSpaceOnUse"
          patternTransform="rotate({item.pattern})"
        >
          <line x1="2" y1="0" x2="2" y2="4" stroke={item.ink} stroke-width="1.6" />
        </pattern>
      </defs>
    {/if}
    <rect width="12" height="12" rx="2" fill={item.fill} />
    {#if item.pattern != null}
      <rect width="12" height="12" rx="2" fill="url(#swatch-{salt}-{item.key})" />
    {/if}
    <rect width="12" height="12" rx="2" fill="none" stroke={theme.grid} />
  </svg>
{/snippet}
