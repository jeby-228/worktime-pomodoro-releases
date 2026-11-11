import { describe, it, expect, vi } from "vitest";
import {
  axisScale,
  buildSeries,
  DARK_CHART,
  escapeXml,
  formatHours,
  LIGHT_CHART,
  loadReportBundle,
  pieChartSvg,
  presetRange,
  reportImage,
  stackedBarSvg,
  summaryParts,
  wrapText,
} from "./report";
import type { Category, CategoryReport } from "./types";

const labels = { uncategorized: "未分類" };

/** Stand-in for get_categories. Names are arbitrary and are not the seeded defaults. */
function makeCategories(count: number): Category[] {
  return Array.from({ length: count }, (_, index) => {
    const id = index + 1;
    return {
      id,
      name: `user-${id}`,
      report_code: null,
      sort_order: id,
      hidden: false,
    };
  });
}

function row(
  id: number,
  name: string,
  totalSeconds: number,
  percentage: number,
  interruptSeconds = 0,
) {
  return {
    category_id: id,
    name,
    total_seconds: totalSeconds,
    interrupt_seconds: interruptSeconds,
    percentage,
  };
}

function week(rows: CategoryReport["rows"]): CategoryReport {
  const total = rows.reduce((sum, item) => sum + item.total_seconds, 0);
  return {
    start_date: "2026-10-26",
    end_date: "2026-11-01",
    total_seconds: total,
    rows,
    days: [
      {
        date: "2026-10-26",
        total_seconds: total,
        items: rows.map((item) => ({
          category_id: item.category_id,
          seconds: item.total_seconds,
          interrupt_seconds: item.interrupt_seconds,
        })),
      },
    ],
  };
}

/** Six categories from get_categories. The last one was added by the user and has the hours. */
function addedCategoryWeek() {
  const list = makeCategories(6);
  const added = list[5];
  const rows = [
    row(list[0].id, list[0].name, 3240, 7),
    row(list[1].id, list[1].name, 2880, 6),
    row(list[2].id, list[2].name, 6480, 13),
    row(added.id, "stale-payload", 37080, 74),
  ];
  return { list, added, data: week(rows) };
}

const categories = makeCategories(2);
const [first, second] = categories;

const report: CategoryReport = {
  start_date: "2026-10-26",
  end_date: "2026-10-27",
  total_seconds: 36000,
  rows: [
    row(first.id, first.name, 18000, 50),
    row(second.id, second.name, 10800, 30, 3600),
    {
      category_id: null,
      name: null,
      total_seconds: 7200,
      interrupt_seconds: 0,
      percentage: 20,
    },
  ],
  days: [
    {
      date: "2026-10-26",
      total_seconds: 36000,
      items: [
        { category_id: first.id, seconds: 18000, interrupt_seconds: 0 },
        { category_id: second.id, seconds: 10800, interrupt_seconds: 3600 },
        { category_id: null, seconds: 7200, interrupt_seconds: 0 },
      ],
    },
    { date: "2026-10-27", total_seconds: 0, items: [] },
  ],
};

describe("presetRange", () => {
  // 2026-11-01 is a Sunday, the last day of a Monday-based week.
  const sunday = new Date(2026, 10, 1);

  it("treats Sunday as the end of this week", () => {
    expect(presetRange("this_week", sunday)).toEqual({ start: "2026-10-26", end: "2026-11-01" });
  });

  it("starts the week on Monday", () => {
    const monday = new Date(2026, 9, 26);
    expect(presetRange("this_week", monday)).toEqual({ start: "2026-10-26", end: "2026-11-01" });
  });

  it("goes back one full week", () => {
    expect(presetRange("last_week", sunday)).toEqual({ start: "2026-10-19", end: "2026-10-25" });
  });

  it("covers the calendar month", () => {
    expect(presetRange("this_month", new Date(2026, 1, 14))).toEqual({
      start: "2026-02-01",
      end: "2026-02-28",
    });
  });
});

describe("numbers", () => {
  it("rounds hours to one decimal", () => {
    expect(formatHours(115200)).toBe("32");
    expect(formatHours(5400)).toBe("1.5");
    expect(formatHours(0)).toBe("0");
  });

  it("picks a clean axis top", () => {
    expect(axisScale(0)).toEqual({ top: 1, step: 1 });
    expect(axisScale(3.2)).toEqual({ top: 4, step: 1 });
    expect(axisScale(7.5)).toEqual({ top: 8, step: 2 });
    expect(axisScale(9)).toEqual({ top: 12, step: 4 });
  });
});

describe("summaryParts", () => {
  it("lists shares largest first and names uncategorized", () => {
    expect(summaryParts(report, categories, "未分類", "、")).toBe(
      `${first.name} 50%、${second.name} 30%、未分類 20%`,
    );
  });

  it("uses the current category name", () => {
    const renamed = [{ ...first, name: `${first.name}-v2` }, second];
    expect(summaryParts(report, renamed, "未分類", "、")).toBe(
      `${first.name}-v2 50%、${second.name} 30%、未分類 20%`,
    );
  });
});

describe("buildSeries", () => {
  it("keeps a category's shade when it drops out of the range", () => {
    const full = buildSeries(report, categories, LIGHT_CHART, labels);
    const onlySecond: CategoryReport = { ...report, rows: [report.rows[1]] };
    const filtered = buildSeries(onlySecond, categories, LIGHT_CHART, labels);
    expect(filtered[0].fill).toBe(full.find((item) => item.name === second.name)?.fill);
  });

  it("puts uncategorized last in its own shade", () => {
    const series = buildSeries(report, categories, LIGHT_CHART, labels);
    expect(series.map((item) => item.name)).toEqual([first.name, second.name, "未分類"]);
    expect(series[2].fill).toBe(LIGHT_CHART.uncategorized);
  });

  it("names every category that has hours, past the base palette", () => {
    const many = makeCategories(7);
    const rows = many.map((item) =>
      row(item.id, item.name, 3600, 100 / 7, item.id === many[6].id ? 600 : 0),
    );
    const data = week(rows);
    const series = buildSeries(data, many, LIGHT_CHART, labels);

    expect(series.map((item) => item.name)).toEqual(many.map((item) => item.name));
    expect(series.some((item) => item.name === "其他")).toBe(false);
    expect(series[5].pattern).toBe(0);
    expect(series[6].pattern).toBe(0);
    expect(series[6].seconds).toBe(3600);
    expect(series[6].interruptSeconds).toBe(600);
    expect(new Set(series.map((item) => `${item.fill}/${item.pattern}`)).size).toBe(7);
    for (const item of series.slice(0, 5)) expect(item.pattern).toBeNull();

    const pie = pieChartSvg(series, LIGHT_CHART);
    const bar = stackedBarSvg(data, series, LIGHT_CHART, "小時");
    const image = reportImage(pie, 1, series, LIGHT_CHART, {
      title: "週會報告",
      summary: "本週",
      note: "不含午休",
    });
    for (const name of [many[0].name, many[5].name, many[6].name]) {
      expect(pie).toContain(name);
      expect(bar).toContain(name);
      expect(image.svg).toContain(name);
    }
    expect(pie).not.toContain("其他");
    expect(image.svg).toContain(`fill="${series[5].fill}"`);
    expect(image.svg).toContain('fill="url(#legend-5)"');
  });

  it("prefers the current category name over a stale report name", () => {
    const list = makeCategories(1);
    const renamed = { ...list[0], name: "after-rename" };
    const rows = [
      { ...report.rows[0], category_id: renamed.id, name: list[0].name, percentage: 100 },
    ];
    const series = buildSeries({ ...report, rows }, [renamed], LIGHT_CHART, labels);
    expect(series.map((item) => item.name)).toEqual([renamed.name]);
    const pie = pieChartSvg(series, LIGHT_CHART);
    expect(pie).toContain(renamed.name);
    expect(pie).not.toContain(list[0].name);
  });

  it("gives visible categories the shades before hidden ones", () => {
    const list = makeCategories(2);
    list[0] = { ...list[0], hidden: true };
    const rows = [{ ...report.rows[0], category_id: list[1].id, name: "stale" }];
    const series = buildSeries({ ...report, rows }, list, LIGHT_CHART, labels);
    expect(series[0].name).toBe(list[1].name);
    expect(series[0].fill).toBe(LIGHT_CHART.shades[0]);
  });

  it("names a later category when it has the most hours", () => {
    const { list, added, data } = addedCategoryWeek();
    const series = buildSeries(data, list, LIGHT_CHART, labels);

    expect(series.map((item) => item.name)).toEqual([
      list[0].name,
      list[1].name,
      list[2].name,
      added.name,
    ]);
    const training = series[3];
    expect(training.seconds).toBe(37080);
    expect(training.percentage).toBe(74);
    expect(training.pattern).toBe(0);
    expect(LIGHT_CHART.shades).not.toContain(training.fill);
    expect(series.find((item) => item.name === list[0].name)?.fill).toBe(LIGHT_CHART.shades[0]);
    expect(series.some((item) => item.name === "其他")).toBe(false);
    expect(series.some((item) => item.name === "stale-payload")).toBe(false);
  });

  it("keeps the extra shade when the theme changes only by surface", () => {
    const list = makeCategories(6);
    const rows = [row(list[5].id, list[5].name, 3600, 100)];
    const light = buildSeries(week(rows), list, LIGHT_CHART, labels);
    const again = buildSeries(week(rows), list, LIGHT_CHART, labels);
    const dark = buildSeries(week(rows), list, DARK_CHART, labels);
    expect(light[0].fill).toBe(again[0].fill);
    expect(light[0].pattern).toBe(0);
    expect(dark[0].fill).not.toBe(light[0].fill);
  });
});

describe("training week on every chart", () => {
  const { list, added, data } = addedCategoryWeek();
  const series = buildSeries(data, list, LIGHT_CHART, labels);
  const training = series.find((item) => item.name === added.name);
  const pie = pieChartSvg(series, LIGHT_CHART);
  const bar = stackedBarSvg(data, series, LIGHT_CHART, "小時");
  const image = reportImage(pie, 1, series, LIGHT_CHART, {
    title: "週會報告",
    summary: `本週共 13.8 小時：${added.name} 74%`,
    note: "不含午休",
  });

  it("uses the category name and the same colour on the pie, bars and PNG", () => {
    expect(training?.pattern).toBe(0);
    expect(pie).toContain(`<title>${added.name} 10.3h</title>`);
    expect(pie).toContain(">74%<");
    expect(pie).toContain('fill="url(#pie-3-mark)"');
    expect(pie).not.toContain("其他");
    expect(pie).not.toContain("stale-payload");
    expect(bar).toContain(`<title>10/26 ${added.name} 10.3h</title>`);
    expect(bar).toContain('fill="url(#bar-3-mark)"');
    expect(bar).not.toContain("其他");
    expect(image.svg).toContain(added.name);
    expect(image.svg).toContain("10.3h · 74%");
    expect(image.svg).toContain('fill="url(#legend-3)"');
    expect(image.svg).not.toContain("其他");
    expect(image.svg).not.toContain("stale-payload");
    for (const svg of [pie, bar, image.svg]) {
      expect(svg).toContain(`fill="${training?.fill}"`);
    }
  });
});

describe("loadReportBundle", () => {
  it("uses the mocked get_categories list, including a rename and a new category", async () => {
    const listed = makeCategories(6);
    const added = listed[5];
    const renamed = { ...listed[0], name: "after-rename" };
    const current = [renamed, ...listed.slice(1)];
    const getCategories = vi.fn(async () => current);
    const stale = week([
      row(listed[0].id, listed[0].name, 3240, 7),
      row(added.id, "stale-payload", 37080, 74),
    ]);
    const getCategoryReport = vi.fn(async () => stale);
    const loaded = await loadReportBundle(
      { getCategories, getCategoryReport },
      "2026-10-26",
      "2026-11-01",
    );

    expect(getCategories).toHaveBeenCalledTimes(1);
    expect(getCategoryReport).toHaveBeenCalledWith("2026-10-26", "2026-11-01");
    const series = buildSeries(loaded.report, loaded.categories, LIGHT_CHART, labels);
    expect(series.map((item) => item.name)).toEqual([renamed.name, added.name]);
    expect(pieChartSvg(series, LIGHT_CHART)).toContain(added.name);
    expect(pieChartSvg(series, LIGHT_CHART)).not.toContain("stale-payload");
    expect(pieChartSvg(series, LIGHT_CHART)).not.toContain(`>${listed[0].name}<`);
  });
});

describe("pieChartSvg", () => {
  const series = buildSeries(report, categories, LIGHT_CHART, labels);

  it("hatches only the interrupt part", () => {
    const svg = pieChartSvg(series, LIGHT_CHART);
    expect(svg.match(/fill="url\(#pie-\d+-45\)"/g)).toHaveLength(1);
  });

  it("labels shares that fit", () => {
    const svg = pieChartSvg(series, LIGHT_CHART);
    expect(svg).toContain(">50%<");
    expect(svg).toContain(">20%<");
  });

  it("draws a single category as a full circle", () => {
    const one = buildSeries(
      { ...report, rows: [{ ...report.rows[0], percentage: 100 }] },
      categories,
      LIGHT_CHART,
      labels,
    );
    expect(pieChartSvg(one, LIGHT_CHART)).toContain("<circle");
  });

  it("escapes category names", () => {
    const list = makeCategories(1);
    list[0] = { ...list[0], name: '<b>"A&B"</b>' };
    const rows = [{ ...report.rows[0], category_id: list[0].id, name: "ignored" }];
    const named = buildSeries({ ...report, rows }, list, LIGHT_CHART, labels);
    const svg = pieChartSvg(named, LIGHT_CHART);
    expect(svg).not.toContain("<b>");
    expect(svg).toContain(escapeXml('<b>"A&B"</b>'));
  });
});

describe("stackedBarSvg", () => {
  const series = buildSeries(report, categories, LIGHT_CHART, labels);
  const svg = stackedBarSvg(report, series, LIGHT_CHART, "小時");

  it("draws one label per day and stacks each category", () => {
    expect(svg).toContain(">10/26<");
    expect(svg).toContain(">10/27<");
    expect(svg.match(/<title>10\/26 /g)?.length).toBeGreaterThanOrEqual(4);
  });

  it("hatches interrupt time", () => {
    expect(svg).toContain('fill="url(#bar-1-45)"');
  });

  it("thins date labels on a long range", () => {
    const days = Array.from({ length: 31 }, (_, index) => ({
      date: `2026-10-${String(index + 1).padStart(2, "0")}`,
      total_seconds: 0,
      items: [],
    }));
    const month = stackedBarSvg({ ...report, days }, [], LIGHT_CHART, "小時");
    expect(month.match(/>10\/\d+</g)?.length).toBeLessThanOrEqual(11);
  });
});

describe("reportImage", () => {
  const series = buildSeries(report, categories, LIGHT_CHART, labels);

  it("carries the summary, legend and note in one standalone image", () => {
    const image = reportImage(pieChartSvg(series, LIGHT_CHART), 1, series, LIGHT_CHART, {
      title: "週會報告",
      summary: `本週共 10 小時：${first.name} 50%`,
      note: "不含午休",
    });
    expect(image.svg.startsWith("<svg")).toBe(true);
    expect(image.svg).toContain("週會報告");
    expect(image.svg).toContain("本週共 10 小時");
    expect(image.svg).toContain("5h · 50%");
    expect(image.svg).toContain("不含午休");
    expect(image.svg).not.toContain("var(--");
  });

  it("wraps a long summary without losing text", () => {
    const text = "alpha 45%, beta 17%".repeat(5);
    const lines = wrapText(text, 20);
    expect(lines.length).toBeGreaterThan(1);
    expect(lines.join("")).toBe(text);
  });
});
