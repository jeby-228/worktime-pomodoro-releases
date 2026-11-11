import type { Category, CategoryReport, CategoryReportDay } from "./types";

export type ReportPreset = "this_week" | "last_week" | "this_month" | "custom";

export function toDateString(date: Date): string {
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

/** Weeks run Monday to Sunday. */
export function presetRange(
  preset: Exclude<ReportPreset, "custom">,
  today: Date,
): { start: string; end: string } {
  const year = today.getFullYear();
  const month = today.getMonth();
  if (preset === "this_month") {
    return {
      start: toDateString(new Date(year, month, 1)),
      end: toDateString(new Date(year, month + 1, 0)),
    };
  }
  const sinceMonday = (today.getDay() + 6) % 7;
  const offset = preset === "last_week" ? 7 : 0;
  const monday = today.getDate() - sinceMonday - offset;
  return {
    start: toDateString(new Date(year, month, monday)),
    end: toDateString(new Date(year, month, monday + 6)),
  };
}

/** Hours to one decimal, without a trailing .0. */
export function formatHours(seconds: number): string {
  return String(Math.round((seconds / 3600) * 10) / 10);
}

export function formatPercent(percentage: number): string {
  return `${Math.round(percentage)}%`;
}

// ==================== Series ====================

export interface ChartTheme {
  surface: string;
  ink: string;
  muted: string;
  grid: string;
  /** Category fills in an order that keeps neighbours apart. */
  shades: string[];
  uncategorized: string;
  other: string;
}

/** Single-tone D-mono ramps, each picked for its own surface. */
export const LIGHT_CHART: ChartTheme = {
  surface: "#ffffff",
  ink: "#09090b",
  muted: "#71717a",
  grid: "#e4e4e7",
  shades: ["#18181b", "#71717a", "#d4d4d8", "#3f3f46", "#a1a1aa"],
  uncategorized: "#e4e4e7",
  other: "#a1a1aa",
};

export const DARK_CHART: ChartTheme = {
  surface: "#09090b",
  ink: "#fafafa",
  muted: "#a1a1aa",
  grid: "#27272a",
  shades: ["#fafafa", "#a1a1aa", "#52525b", "#d4d4d8", "#71717a"],
  uncategorized: "#3f3f46",
  other: "#71717a",
};

export interface ChartSeries {
  key: string;
  name: string;
  fill: string;
  /** Ink that stays visible on `fill`, used for hatches. */
  ink: string;
  /** Hatch angle for categories past the base shades. Null is a solid swatch. */
  pattern: number | null;
  seconds: number;
  interruptSeconds: number;
  percentage: number;
}

export interface SeriesLabels {
  uncategorized: string;
}

const UNCATEGORIZED_KEY = "none";
/** Not 45°: that angle is reserved for interrupt time. */
const MARK_ANGLES = [0, 90, 30, -30, 60, -60];

/**
 * Colour follows the category's place in the current list, not how many hours
 * it has, so it stays put when the range changes. Visible categories come
 * first. Past the base shades, greys continue and a hatch keeps them apart.
 */
function categoryLooks(categories: Category[], theme: ChartTheme) {
  const ordered = [...categories.filter((c) => !c.hidden), ...categories.filter((c) => c.hidden)];
  const order = new Map(ordered.map((category, index) => [category.id, index]));
  const fills = new Map<number, string>();
  const patterns = new Map<number, number | null>();
  ordered.forEach((category, index) => {
    const look = lookFor(index, theme);
    fills.set(category.id, look.fill);
    patterns.set(category.id, look.pattern);
  });
  return { fills, patterns, order };
}

function lookFor(index: number, theme: ChartTheme): { fill: string; pattern: number | null } {
  if (index < theme.shades.length) return { fill: theme.shades[index], pattern: null };
  const fills = extraFills(theme);
  const extra = index - theme.shades.length;
  const fill = fills[extra % fills.length] ?? theme.shades[extra % theme.shades.length];
  const pattern = MARK_ANGLES[Math.floor(extra / fills.length) % MARK_ANGLES.length];
  return { fill, pattern };
}

function extraFills(theme: ChartTheme): string[] {
  const banned = [theme.surface, theme.ink, theme.uncategorized, theme.other, ...theme.shades].map(
    (hex) => parseInt(hex.slice(1, 3), 16),
  );
  const onDark = banned[0] < 128;
  const fills: string[] = [];
  const start = onDark ? 48 : 36;
  const end = onDark ? 236 : 200;
  for (let value = start; value <= end && fills.length < 12; value += 8) {
    const crowded = [...banned, ...fills.map((hex) => parseInt(hex.slice(1, 3), 16))].some(
      (byte) => Math.abs(byte - value) < 10,
    );
    if (crowded) continue;
    const channel = value.toString(16).padStart(2, "0");
    fills.push(`#${channel}${channel}${channel}`);
  }
  if (fills.length === 0) fills.push(onDark ? "#888888" : "#555555");
  return fills;
}

function categoryName(
  id: number | null,
  categories: Category[],
  fallback: string | null,
  uncategorized: string,
): string {
  if (id === null) return uncategorized;
  return categories.find((category) => category.id === id)?.name ?? fallback ?? uncategorized;
}

export function buildSeries(
  report: CategoryReport,
  categories: Category[],
  theme: ChartTheme,
  labels: SeriesLabels,
): ChartSeries[] {
  const { fills, patterns, order } = categoryLooks(categories, theme);
  const merged = new Map<string, ChartSeries & { rank: number }>();

  for (const row of report.rows) {
    if (row.total_seconds <= 0) continue;
    const isNone = row.category_id === null;
    const key = isNone ? UNCATEGORIZED_KEY : `c${row.category_id}`;
    const existing = merged.get(key);
    if (existing) {
      existing.seconds += row.total_seconds;
      existing.interruptSeconds += row.interrupt_seconds;
      existing.percentage += row.percentage;
      continue;
    }
    const id = row.category_id;
    const look =
      id !== null && fills.has(id)
        ? { fill: fills.get(id) as string, pattern: patterns.get(id) ?? null }
        : lookFor(theme.shades.length + (id ?? 0), theme);
    const fill = isNone ? theme.uncategorized : look.fill;
    merged.set(key, {
      key,
      name: categoryName(id, categories, row.name, labels.uncategorized),
      fill,
      ink: inkOn(fill),
      pattern: isNone ? null : look.pattern,
      seconds: row.total_seconds,
      interruptSeconds: row.interrupt_seconds,
      percentage: row.percentage,
      rank: isNone
        ? Number.MAX_SAFE_INTEGER
        : (order.get(id as number) ?? Number.MAX_SAFE_INTEGER - 1),
    });
  }

  return [...merged.values()]
    .sort((a, b) => a.rank - b.rank)
    .map(({ rank: _rank, ...series }) => series);
}

export interface ReportLoader {
  getCategoryReport: (start: string, end: string) => Promise<CategoryReport>;
  getCategories: () => Promise<Category[]>;
}

/** Reads the report and the current category list together. Callers pass get_categories. */
export async function loadReportBundle(
  loader: ReportLoader,
  start: string,
  end: string,
): Promise<{ report: CategoryReport; categories: Category[] }> {
  const [report, categories] = await Promise.all([
    loader.getCategoryReport(start, end),
    loader.getCategories(),
  ]);
  return { report, categories };
}

/** The "Name 45%, Name 17%" part of the summary, largest first. */
export function summaryParts(
  report: CategoryReport,
  categories: Category[],
  uncategorized: string,
  separator: string,
): string {
  return report.rows
    .map(
      (row) =>
        `${categoryName(row.category_id, categories, row.name, uncategorized)} ${formatPercent(row.percentage)}`,
    )
    .join(separator);
}

// ==================== SVG ====================

const FONT = "'Noto Sans TC', system-ui, sans-serif";

export function escapeXml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function luminance(hex: string): number {
  const channel = (offset: number) => {
    const value = parseInt(hex.slice(offset, offset + 2), 16) / 255;
    return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
}

function inkOn(fill: string): string {
  return luminance(fill) > 0.25 ? "#09090b" : "#fafafa";
}

function round(value: number): string {
  return String(Math.round(value * 100) / 100);
}

function stripes(id: string, item: ChartSeries, angle: number, width = 6): string {
  return (
    `<pattern id="${id}" width="${width}" height="${width}" patternUnits="userSpaceOnUse" patternTransform="rotate(${angle})">` +
    `<line x1="${width / 2}" y1="0" x2="${width / 2}" y2="${width}" stroke="${inkOn(item.fill)}" stroke-width="1.6"/>` +
    `</pattern>`
  );
}

/** Interrupt stripes, plus a category hatch once the base shades run out. */
function patternDefs(series: ChartSeries[], prefix: string): string {
  const defs = series.flatMap((item, index) => {
    const out = [stripes(`${prefix}-${index}-45`, item, 45)];
    if (item.pattern != null) out.push(stripes(`${prefix}-${index}-mark`, item, item.pattern));
    return out;
  });
  return `<defs>${defs.join("")}</defs>`;
}

/** Paints a shape in the series fill, then lays its hatching on top. */
function paint(
  shape: (fill: string) => string,
  item: ChartSeries,
  index: number,
  prefix: string,
  isInterrupt: boolean,
): string {
  const layers = [shape(item.fill)];
  if (item.pattern != null) layers.push(shape(`url(#${prefix}-${index}-mark)`));
  if (isInterrupt) layers.push(shape(`url(#${prefix}-${index}-45)`));
  return layers.join("");
}

interface Part {
  item: ChartSeries;
  index: number;
  isInterrupt: boolean;
  seconds: number;
}

function partsOf(item: ChartSeries, index: number, seconds: number, interrupt: number): Part[] {
  const parts: Part[] = [];
  const plain = seconds - interrupt;
  if (plain > 0) parts.push({ item, index, isInterrupt: false, seconds: plain });
  if (interrupt > 0) parts.push({ item, index, isInterrupt: true, seconds: interrupt });
  return parts;
}

const PIE_SIZE = 320;

export function pieChartSvg(series: ChartSeries[], theme: ChartTheme, prefix = "pie"): string {
  const total = series.reduce((sum, item) => sum + item.seconds, 0);
  const center = PIE_SIZE / 2;
  const radius = center - 8;
  const body: string[] = [];
  const labels: string[] = [];
  let angle = -Math.PI / 2;

  const point = (at: number, distance: number) =>
    `${round(center + distance * Math.cos(at))} ${round(center + distance * Math.sin(at))}`;

  series.forEach((item, index) => {
    const seriesStart = angle;
    for (const part of partsOf(item, index, item.seconds, item.interruptSeconds)) {
      const sweep = (part.seconds / total) * Math.PI * 2;
      const end = angle + sweep;
      const large = sweep > Math.PI ? 1 : 0;
      const start = angle;
      const title = `<title>${escapeXml(item.name)} ${formatHours(part.seconds)}h</title>`;
      const shape =
        sweep >= Math.PI * 2 - 0.0001
          ? (fill: string) =>
              `<circle cx="${center}" cy="${center}" r="${radius}" fill="${fill}">${title}</circle>`
          : (fill: string) =>
              `<path d="M${center} ${center} L${point(start, radius)} A${radius} ${radius} 0 ${large} 1 ${point(end, radius)} Z" ` +
              `fill="${fill}" stroke="${theme.surface}" stroke-width="2" stroke-linejoin="round">${title}</path>`;
      body.push(paint(shape, item, index, prefix, part.isInterrupt));
      angle = end;
    }
    // A share only gets a label when it fits inside its slice.
    if (item.percentage >= 8) {
      const [x, y] = point((seriesStart + angle) / 2, radius * 0.66).split(" ");
      const ink = inkOn(item.fill);
      labels.push(
        `<text x="${x}" y="${y}" text-anchor="middle" dominant-baseline="central" font-size="14" font-weight="600" ` +
          `fill="${ink}" stroke="${item.fill}" stroke-width="4" paint-order="stroke">${formatPercent(item.percentage)}</text>`,
      );
    }
  });

  return (
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${PIE_SIZE} ${PIE_SIZE}" width="100%" role="img" font-family="${FONT}">` +
    patternDefs(series, prefix) +
    body.join("") +
    labels.join("") +
    `</svg>`
  );
}

const BAR_WIDTH = 640;
const BAR_HEIGHT = 300;
const BAR_MARGIN = { top: 12, right: 8, bottom: 28, left: 40 };

/** A clean axis top in hours, with at most four steps. */
export function axisScale(maxHours: number): { top: number; step: number } {
  for (const step of [1, 2, 4, 5, 10, 20, 25, 50, 100, 200, 500]) {
    const ticks = Math.ceil(maxHours / step);
    if (ticks <= 4) return { top: Math.max(1, ticks) * step, step };
  }
  const step = Math.ceil(maxHours / 4);
  return { top: step * 4, step };
}

function shortDate(date: string): string {
  const [, month, day] = date.split("-").map(Number);
  return `${month}/${day}`;
}

function dayParts(day: CategoryReportDay, series: ChartSeries[]) {
  const seconds = new Map<string, { total: number; interrupt: number }>();
  for (const item of day.items) {
    const key = item.category_id === null ? UNCATEGORIZED_KEY : `c${item.category_id}`;
    const entry = seconds.get(key) ?? { total: 0, interrupt: 0 };
    entry.total += item.seconds;
    entry.interrupt += item.interrupt_seconds;
    seconds.set(key, entry);
  }
  return series.flatMap((item, index) => {
    const entry = seconds.get(item.key);
    return entry ? partsOf(item, index, entry.total, entry.interrupt) : [];
  });
}

export function stackedBarSvg(
  report: CategoryReport,
  series: ChartSeries[],
  theme: ChartTheme,
  hoursLabel: string,
  prefix = "bar",
): string {
  const plotWidth = BAR_WIDTH - BAR_MARGIN.left - BAR_MARGIN.right;
  const plotHeight = BAR_HEIGHT - BAR_MARGIN.top - BAR_MARGIN.bottom;
  const baseline = BAR_MARGIN.top + plotHeight;
  const days = report.days;
  const maxHours = Math.max(...days.map((day) => day.total_seconds), 0) / 3600;
  const { top, step } = axisScale(maxHours);
  const band = plotWidth / Math.max(days.length, 1);
  const barWidth = Math.min(24, band * 0.7);
  const labelEvery = Math.ceil(days.length / 10);
  const out: string[] = [];

  for (let tick = 0; tick <= top; tick += step) {
    const y = baseline - (tick / top) * plotHeight;
    out.push(
      `<line x1="${BAR_MARGIN.left}" y1="${round(y)}" x2="${BAR_WIDTH - BAR_MARGIN.right}" y2="${round(y)}" stroke="${theme.grid}" stroke-width="1"/>`,
      `<text x="${BAR_MARGIN.left - 8}" y="${round(y)}" text-anchor="end" dominant-baseline="central" font-size="12" fill="${theme.muted}">${tick}</text>`,
    );
  }
  out.push(
    `<text x="0" y="8" font-size="11" fill="${theme.muted}">${escapeXml(hoursLabel)}</text>`,
  );

  days.forEach((day, dayIndex) => {
    const x = BAR_MARGIN.left + band * dayIndex + (band - barWidth) / 2;
    const parts = dayParts(day, series);
    let y = baseline;
    parts.forEach((part, partIndex) => {
      const height = (part.seconds / 3600 / top) * plotHeight;
      y -= height;
      const isTop = partIndex === parts.length - 1;
      const drawn = Math.max(1, height - (isTop ? 0 : 2));
      const yTop = y + (isTop ? 0 : 2);
      const title = `<title>${shortDate(day.date)} ${escapeXml(part.item.name)} ${formatHours(part.seconds)}h</title>`;
      const corner = isTop ? Math.min(4, drawn, barWidth / 2) : 0;
      const right = x + barWidth;
      const bottom = yTop + drawn;
      const d =
        `M${round(x)} ${round(bottom)} V${round(yTop + corner)} ` +
        `Q${round(x)} ${round(yTop)} ${round(x + corner)} ${round(yTop)} ` +
        `H${round(right - corner)} Q${round(right)} ${round(yTop)} ${round(right)} ${round(yTop + corner)} ` +
        `V${round(bottom)} Z`;
      out.push(
        paint(
          (fill) => `<path d="${d}" fill="${fill}">${title}</path>`,
          part.item,
          part.index,
          prefix,
          part.isInterrupt,
        ),
      );
    });
    if (dayIndex % labelEvery === 0) {
      out.push(
        `<text x="${round(x + barWidth / 2)}" y="${BAR_HEIGHT - 8}" text-anchor="middle" font-size="12" fill="${theme.muted}">${shortDate(day.date)}</text>`,
      );
    }
  });

  return (
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${BAR_WIDTH} ${BAR_HEIGHT}" width="100%" role="img" font-family="${FONT}">` +
    patternDefs(series, prefix) +
    out.join("") +
    `</svg>`
  );
}

// ==================== Image export ====================

/** Splits text to fit a width. CJK counts as one unit, Latin as about half. */
export function wrapText(text: string, maxUnits: number): string[] {
  const lines: string[] = [];
  let line = "";
  let units = 0;
  for (const char of text) {
    const width = char.charCodeAt(0) > 0x2e7f ? 1 : 0.55;
    if (units + width > maxUnits) {
      lines.push(line);
      line = "";
      units = 0;
    }
    line += char;
    units += width;
  }
  if (line) lines.push(line);
  return lines;
}

export interface ReportImage {
  svg: string;
  width: number;
  height: number;
}

export interface ReportImageText {
  title: string;
  summary: string;
  note: string;
}

/** One self-contained image with the title, chart, legend and footnote. */
export function reportImage(
  chartSvg: string,
  chartHeightRatio: number,
  series: ChartSeries[],
  theme: ChartTheme,
  text: ReportImageText,
): ReportImage {
  const width = 960;
  const padding = 32;
  const legendWidth = 280;
  const chartWidth = width - padding * 3 - legendWidth;
  const chartHeight = Math.round(chartWidth * chartHeightRatio);
  const summaryLines = wrapText(text.summary, 50);
  const chartTop = padding + 44 + summaryLines.length * 24;
  const legendHeight = series.length * 28;
  const bodyHeight = Math.max(chartHeight, legendHeight);
  const height = chartTop + bodyHeight + 56;
  const legendX = padding * 2 + chartWidth;

  const summary = summaryLines
    .map(
      (line, index) =>
        `<text x="${padding}" y="${padding + 50 + index * 24}" font-size="15" fill="${theme.ink}">${escapeXml(line)}</text>`,
    )
    .join("");

  const legendDefs = series
    .map((item, index) =>
      item.pattern == null ? "" : stripes(`legend-${index}`, item, item.pattern, 4),
    )
    .join("");
  const legend = series
    .map((item, index) => {
      const y = chartTop + index * 28;
      const mark =
        item.pattern == null
          ? ""
          : `<rect x="${legendX}" y="${y + 4}" width="12" height="12" rx="2" fill="url(#legend-${index})"/>`;
      return (
        `<rect x="${legendX}" y="${y + 4}" width="12" height="12" rx="2" fill="${item.fill}" stroke="${theme.grid}"/>` +
        mark +
        `<text x="${legendX + 20}" y="${y + 15}" font-size="14" fill="${theme.ink}">${escapeXml(item.name)}</text>` +
        `<text x="${width - padding}" y="${y + 15}" text-anchor="end" font-size="14" fill="${theme.muted}">${formatHours(item.seconds)}h · ${formatPercent(item.percentage)}</text>`
      );
    })
    .join("");

  const chart = chartSvg
    .replace("<svg ", `<svg x="${padding}" y="${chartTop}" height="${chartHeight}" `)
    .replace('width="100%"', `width="${chartWidth}"`);

  return {
    width,
    height,
    svg:
      `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" font-family="${FONT}">` +
      `<defs>${legendDefs}</defs>` +
      `<rect width="${width}" height="${height}" fill="${theme.surface}"/>` +
      `<text x="${padding}" y="${padding + 16}" font-size="20" font-weight="600" fill="${theme.ink}">${escapeXml(text.title)}</text>` +
      summary +
      chart +
      legend +
      `<text x="${padding}" y="${height - padding + 8}" font-size="12" fill="${theme.muted}">${escapeXml(text.note)}</text>` +
      `</svg>`,
  };
}

/** Browser only. Draws the SVG at twice its size so it stays sharp when pasted. */
export function imageToPng({ svg, width, height }: ReportImage): Promise<Blob> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.addEventListener(
      "load",
      () => {
        const canvas = document.createElement("canvas");
        canvas.width = width * 2;
        canvas.height = height * 2;
        const context = canvas.getContext("2d");
        if (!context) {
          reject(new Error("Canvas is not available"));
          return;
        }
        context.drawImage(image, 0, 0, canvas.width, canvas.height);
        canvas.toBlob((blob) => {
          if (blob) resolve(blob);
          else reject(new Error("Could not create the PNG"));
        }, "image/png");
      },
      { once: true },
    );
    image.addEventListener("error", () => reject(new Error("Could not render the chart")), {
      once: true,
    });
    image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
  });
}
