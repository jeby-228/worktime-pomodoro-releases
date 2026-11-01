import { describe, it, expect } from "vitest";
import {
  axisScale,
  buildSeries,
  escapeXml,
  formatHours,
  LIGHT_CHART,
  pieChartSvg,
  presetRange,
  reportImage,
  stackedBarSvg,
  summaryParts,
  wrapText,
} from "./report";
import type { Category, CategoryReport } from "./types";

const labels = { uncategorized: "未分類", other: "其他" };

function category(id: number, name: string, hidden = false): Category {
  return { id, name, report_code: null, sort_order: id, hidden };
}

const categories = [category(1, "實作"), category(2, "問題處理")];

const report: CategoryReport = {
  start_date: "2026-10-26",
  end_date: "2026-10-27",
  total_seconds: 36000,
  rows: [
    {
      category_id: 1,
      name: "實作",
      total_seconds: 18000,
      interrupt_seconds: 0,
      percentage: 50,
    },
    {
      category_id: 2,
      name: "問題處理",
      total_seconds: 10800,
      interrupt_seconds: 3600,
      percentage: 30,
    },
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
        { category_id: 1, seconds: 18000, interrupt_seconds: 0 },
        { category_id: 2, seconds: 10800, interrupt_seconds: 3600 },
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
    expect(summaryParts(report, "未分類", "、")).toBe("實作 50%、問題處理 30%、未分類 20%");
  });
});

describe("buildSeries", () => {
  it("keeps a category's shade when it drops out of the range", () => {
    const full = buildSeries(report, categories, LIGHT_CHART, labels);
    const onlySecond: CategoryReport = { ...report, rows: [report.rows[1]] };
    const filtered = buildSeries(onlySecond, categories, LIGHT_CHART, labels);
    expect(filtered[0].fill).toBe(full.find((item) => item.name === "問題處理")?.fill);
  });

  it("puts uncategorized last in its own shade", () => {
    const series = buildSeries(report, categories, LIGHT_CHART, labels);
    expect(series.map((item) => item.name)).toEqual(["實作", "問題處理", "未分類"]);
    expect(series[2].fill).toBe(LIGHT_CHART.uncategorized);
  });

  it("folds categories past the last shade into one series", () => {
    const many = Array.from({ length: 7 }, (_, index) => category(index + 1, `類別${index + 1}`));
    const rows = many.map((item) => ({
      category_id: item.id,
      name: item.name,
      total_seconds: 3600,
      interrupt_seconds: item.id === 7 ? 600 : 0,
      percentage: 100 / 7,
    }));
    const series = buildSeries({ ...report, rows }, many, LIGHT_CHART, labels);

    expect(series).toHaveLength(6);
    const other = series[5];
    expect(other.name).toBe("其他");
    expect(other.isOther).toBe(true);
    expect(other.seconds).toBe(7200);
    expect(other.interruptSeconds).toBe(600);
  });

  it("gives visible categories the shades before hidden ones", () => {
    const list = [category(1, "舊類別", true), category(2, "實作")];
    const rows = [{ ...report.rows[0], category_id: 2, name: "實作" }];
    const series = buildSeries({ ...report, rows }, list, LIGHT_CHART, labels);
    expect(series[0].fill).toBe(LIGHT_CHART.shades[0]);
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
    const rows = [{ ...report.rows[0], name: '<b>"A&B"</b>' }];
    const named = buildSeries({ ...report, rows }, categories, LIGHT_CHART, labels);
    const svg = pieChartSvg(named, LIGHT_CHART);
    expect(svg).not.toContain("<b>");
    expect(svg).toContain(escapeXml('<b>"A&B"</b>'));
  });
});

describe("stackedBarSvg", () => {
  const series = buildSeries(report, categories, LIGHT_CHART, labels);
  const svg = stackedBarSvg(report, series, categories, LIGHT_CHART, "小時");

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
    const month = stackedBarSvg({ ...report, days }, [], categories, LIGHT_CHART, "小時");
    expect(month.match(/>10\/\d+</g)?.length).toBeLessThanOrEqual(11);
  });
});

describe("reportImage", () => {
  const series = buildSeries(report, categories, LIGHT_CHART, labels);

  it("carries the summary, legend and note in one standalone image", () => {
    const image = reportImage(pieChartSvg(series, LIGHT_CHART), 1, series, LIGHT_CHART, {
      title: "週會報告",
      summary: "本週共 10 小時：實作 50%",
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
    const text = "實作 45%、問題處理 17%".repeat(5);
    const lines = wrapText(text, 20);
    expect(lines.length).toBeGreaterThan(1);
    expect(lines.join("")).toBe(text);
  });
});
