import path from "node:path";
import {createReadStream} from "node:fs";
import {readdir, readFile} from "node:fs/promises";
import {createInterface} from "node:readline";
import {createCanvas} from "@napi-rs/canvas";
import {Font as BdfFont, type Bitmap as BdfBitmap} from "bdfparser";
import {statements, database} from "./queries.ts";

const loadBdfFont = async (filePath: string) => {
  const font = new BdfFont();
  const lines = createInterface({
    input: createReadStream(filePath),
    crlfDelay: Infinity,
  });

  try {
    await font.load_filelines(lines[Symbol.asyncIterator]());
  } finally {
    lines.close();
  }

  return font;
};

type LoadedBitmapFont = {
  font: BdfFont;
  nativeSize: number;
  weight: FontWeight;
  filePath: string;
};

const terminusFontsDirectory = path.join(import.meta.dirname, "fonts/terminus");

const loadBitmapFont = async (filePath: string): Promise<LoadedBitmapFont> => {
  const [font, source] = await Promise.all([
    loadBdfFont(filePath),
    readFile(filePath, "utf8"),
  ]);

  const pixelSizeMatch = source.match(/^PIXEL_SIZE\s+(\d+)$/m);
  const weightMatch = source.match(/^WEIGHT_NAME\s+"([^"]+)"$/m);

  if (!pixelSizeMatch || !weightMatch) {
    throw new Error(`Missing PIXEL_SIZE or WEIGHT_NAME in ${filePath}`);
  }

  return {
    font,
    nativeSize: Number.parseInt(pixelSizeMatch[1], 10),
    weight: weightMatch[1].toLowerCase() === "bold" ? "bold" : "regular",
    filePath,
  };
};

const terminusFontFiles = (await readdir(terminusFontsDirectory))
  .filter((entry) => entry.toLowerCase().endsWith(".bdf"))
  .sort();

const loadedTerminusFonts = await Promise.all(
  terminusFontFiles.map((fileName) => loadBitmapFont(path.join(terminusFontsDirectory, fileName))),
);

type InfoPanelRow = {
  state_of_charge: number | null;
  state_of_charge_timestamp: number | null;
  pv_sum_watt: number | null;
  pv_sum_watt_timestamp: number | null;
  battery_charging_watt: number | null;
  battery_charging_watt_timestamp: number | null;
  pv_energy_day_wh: number | null;
  pv_energy_day_wh_timestamp: number | null;
  pv_energy_week_wh: number | null;
  pv_energy_week_wh_timestamp: number | null;
  pv_energy_month_wh: number | null;
  pv_energy_month_wh_timestamp: number | null;
  pv_energy_year_wh: number | null;
  pv_energy_year_wh_timestamp: number | null;
  mos_temperature: number | null;
  mos_temperature_timestamp: number | null;
  battery_temperature_1: number | null;
  battery_temperature_1_timestamp: number | null;
  battery_temperature_2: number | null;
  battery_temperature_2_timestamp: number | null;
  battery_temperature_3: number | null;
  battery_temperature_3_timestamp: number | null;
  battery_temperature_4: number | null;
  battery_temperature_4_timestamp: number | null;
  battery_temperature_5: number | null;
  battery_temperature_5_timestamp: number | null;
};

type InfoPanelView = {
  footerTime: Date;
  soc?: number;
  chargingWatt?: number;
  socText: string;
  pvSumWattText: string;
  chargingWattText: string;
  temperatureText: string;
  pvEnergyDayText: string;
  pvEnergyWeekText: string;
  pvEnergyMonthText: string;
  pvEnergyYearText: string;
};

const twoDigits = (n: number) => String(n).padStart(2, "0");

type Box = {
  x: number;
  y: number;
  width: number;
  height: number;
};

type AlignX = "left" | "center" | "right";
type AlignY = "top" | "center" | "bottom";
type FontWeight = "regular" | "bold";
type VerticalMetric = "ink" | "cell";

type BitmapStyle = {
  font: BdfFont;
  nativeSize: number;
  scale: number;
  effectiveSize: number;
  priority: number;
  weight: FontWeight;
};

type MeasuredBitmapText = {
  bitmap: BdfBitmap;
  style: BitmapStyle;
  width: number;
  height: number;
  cellWidth: number;
  cellHeight: number;
  inkLeft: number;
  inkTop: number;
  inkRight: number;
  inkBottom: number;
};

type FittedBitmapText = {
  text: string;
  color: string;
  x: number;
  y: number;
  left: number;
  right: number;
  top: number;
  bottom: number;
  width: number;
  height: number;
  measure: MeasuredBitmapText;
};

type Surface = {
  width: number;
  height: number;
  data: Uint8ClampedArray;
};

type RenderedStrip = {
  surface: Surface;
  bounds: {
    left: number;
    right: number;
    top: number;
    bottom: number;
    width: number;
    height: number;
  };
};

type Strip = {
  height: number;
  render: (width: number) => RenderedStrip;
  flexible?: boolean;
};

const getInfoPanelView = (panelTime: Date, row: InfoPanelRow | null): InfoPanelView => {
  let footerTime = panelTime;

  if (row !== null) {
    const timestamps = Object.entries(row)
      .filter(([key, value]) => key.endsWith("_timestamp") && typeof value === "number")
      .map(([, value]) => value);

    if (timestamps.length > 0) {
      footerTime = new Date(Math.max(...timestamps));
    }
  }

  const soc = row?.state_of_charge === null || row?.state_of_charge === undefined ? undefined : Math.round(row.state_of_charge);
  const pvSumWatt = row?.pv_sum_watt === null || row?.pv_sum_watt === undefined ? undefined : Math.round(row.pv_sum_watt);
  const chargingWatt = row?.battery_charging_watt === null || row?.battery_charging_watt === undefined ? undefined : Math.round(row.battery_charging_watt);
  const pvEnergyDayKwh = row?.pv_energy_day_wh === null || row?.pv_energy_day_wh === undefined ? undefined : row.pv_energy_day_wh / 1000;
  const pvEnergyWeekKwh = row?.pv_energy_week_wh === null || row?.pv_energy_week_wh === undefined ? undefined : row.pv_energy_week_wh / 1000;
  const pvEnergyMonthKwh = row?.pv_energy_month_wh === null || row?.pv_energy_month_wh === undefined ? undefined : row.pv_energy_month_wh / 1000;
  const pvEnergyYearKwh = row?.pv_energy_year_wh === null || row?.pv_energy_year_wh === undefined ? undefined : row.pv_energy_year_wh / 1000;
  const temperatures = row === null
    ? []
    : [
      row.mos_temperature,
      row.battery_temperature_1,
      row.battery_temperature_2,
      row.battery_temperature_3,
      row.battery_temperature_4,
      row.battery_temperature_5,
    ].filter((value): value is number => value !== null);

  const minTemperature = temperatures.length > 0 ? Math.min(...temperatures) : undefined;
  const maxTemperature = temperatures.length > 0 ? Math.max(...temperatures) : undefined;

  return {
    footerTime,
    soc,
    chargingWatt,
    socText: soc === undefined ? "--%" : `${soc}%`,
    pvSumWattText: pvSumWatt === undefined ? "PV -- W" : `PV ${pvSumWatt} W`,
    chargingWattText: chargingWatt === undefined ? "-- W" : `${chargingWatt} W`,
    temperatureText: minTemperature === undefined || maxTemperature === undefined
      ? "--.- °C - --.- °C"
      : `${minTemperature.toFixed(1)} °C - ${maxTemperature.toFixed(1)} °C`,
    pvEnergyDayText: pvEnergyDayKwh === undefined ? "N --" : `N ${pvEnergyDayKwh.toFixed(1)}`,
    pvEnergyWeekText: pvEnergyWeekKwh === undefined ? "He --" : `He ${pvEnergyWeekKwh.toFixed(1)}`,
    pvEnergyMonthText: pvEnergyMonthKwh === undefined ? "H --" : `H ${pvEnergyMonthKwh.toFixed(1)}`,
    pvEnergyYearText: pvEnergyYearKwh === undefined ? "É --" : `É ${pvEnergyYearKwh.toFixed(1)}`,
  };
};

export const runInfoPanelQuery = (panelTime: Date): InfoPanelRow | null => {
  const to = panelTime.getTime();
  const params = {
    from: to - (15 * 60 * 1000),
    to,
  };
  const sql = statements.info_panel();
  const rows = database.prepare(sql + ";").all(params) as InfoPanelRow[];
  if (rows.length === 0) {
    return null;
  }
  return rows[0];
};

const createSurface = (width: number, height: number): Surface => ({
  width,
  height,
  data: new Uint8ClampedArray(width * height * 4),
});

const parseHexColor = (hexColor: string) => {
  const value = hexColor.startsWith("#") ? hexColor.slice(1) : hexColor;
  return {
    r: Number.parseInt(value.slice(0, 2), 16),
    g: Number.parseInt(value.slice(2, 4), 16),
    b: Number.parseInt(value.slice(4, 6), 16),
    a: 255,
  };
};

const blendPixel = (surface: Surface, x: number, y: number, color: {r: number; g: number; b: number; a: number}) => {
  if (x < 0 || y < 0 || x >= surface.width || y >= surface.height) {
    return;
  }
  const offset = ((y * surface.width) + x) * 4;
  const alpha = color.a / 255;
  const inverseAlpha = 1 - alpha;

  surface.data[offset] = Math.round((color.r * alpha) + (surface.data[offset] * inverseAlpha));
  surface.data[offset + 1] = Math.round((color.g * alpha) + (surface.data[offset + 1] * inverseAlpha));
  surface.data[offset + 2] = Math.round((color.b * alpha) + (surface.data[offset + 2] * inverseAlpha));
  surface.data[offset + 3] = Math.round((255 * alpha) + (surface.data[offset + 3] * inverseAlpha));
};

const blitSurface = (target: Surface, source: Surface, x: number, y: number) => {
  for (let sourceY = 0; sourceY < source.height; sourceY += 1) {
    for (let sourceX = 0; sourceX < source.width; sourceX += 1) {
      const sourceOffset = ((sourceY * source.width) + sourceX) * 4;
      const alpha = source.data[sourceOffset + 3];
      if (alpha === 0) {
        continue;
      }
      blendPixel(target, x + sourceX, y + sourceY, {
        r: source.data[sourceOffset],
        g: source.data[sourceOffset + 1],
        b: source.data[sourceOffset + 2],
        a: alpha,
      });
    }
  }
};

const toRgb565 = (surface: Surface) => {
  const rgb565 = Buffer.alloc(surface.width * surface.height * 2);

  for (let i = 0, j = 0; i < surface.data.length; i += 4, j += 2) {
    const r = surface.data[i];
    const g = surface.data[i + 1];
    const b = surface.data[i + 2];
    const c = ((r & 0xf8) << 8) | ((g & 0xfc) << 3) | (b >> 3);
    rgb565[j] = (c >> 8) & 0xff;
    rgb565[j + 1] = c & 0xff;
  }

  return rgb565;
};

const nativeTerminusStyles: readonly BitmapStyle[] = loadedTerminusFonts.map((loadedFont) => ({
  font: loadedFont.font,
  nativeSize: loadedFont.nativeSize,
  scale: 1,
  effectiveSize: loadedFont.nativeSize,
  priority: 0,
  weight: loadedFont.weight,
}));

const largestNativeSizeByWeight = {
  regular: Math.max(...nativeTerminusStyles.filter((style) => style.weight === "regular").map((style) => style.nativeSize)),
  bold: Math.max(...nativeTerminusStyles.filter((style) => style.weight === "bold").map((style) => style.nativeSize)),
} as const;

const scaledTerminusStyles: readonly BitmapStyle[] = loadedTerminusFonts
  .filter((loadedFont) => loadedFont.nativeSize * 2 > largestNativeSizeByWeight[loadedFont.weight])
  .map((loadedFont) => ({
    font: loadedFont.font,
    nativeSize: loadedFont.nativeSize,
    scale: 2,
    effectiveSize: loadedFont.nativeSize * 2,
    priority: 1,
    weight: loadedFont.weight,
  }));

const terminusStyles: readonly BitmapStyle[] = [...nativeTerminusStyles, ...scaledTerminusStyles];

const uniqueSortedStyles = (styles: readonly BitmapStyle[]) => [...styles].sort((a, b) => {
  if (a.effectiveSize !== b.effectiveSize) {
    return b.effectiveSize - a.effectiveSize;
  }
  if (a.priority !== b.priority) {
    return a.priority - b.priority;
  }
  return b.nativeSize - a.nativeSize;
});

const measureBitmapText = (text: string, style: BitmapStyle): MeasuredBitmapText => {
  const bitmap = style.font.draw(text, {usecurrentglyphspacing: true});
  const rows = bitmap.todata(2) as number[][];

  let inkTop = -1;
  let inkBottom = -1;
  let inkLeft = bitmap.width();
  let inkRight = -1;

  for (let rowIndex = 0; rowIndex < rows.length; rowIndex += 1) {
    const row = rows[rowIndex];
    for (let columnIndex = 0; columnIndex < row.length; columnIndex += 1) {
      if (row[columnIndex] === 0) {
        continue;
      }
      if (inkTop === -1) {
        inkTop = rowIndex;
      }
      inkBottom = rowIndex;
      inkLeft = Math.min(inkLeft, columnIndex);
      inkRight = Math.max(inkRight, columnIndex);
    }
  }

  if (inkTop === -1) {
    inkTop = 0;
    inkBottom = 0;
    inkLeft = 0;
    inkRight = 0;
  }

  return {
    bitmap,
    style,
    width: (inkRight - inkLeft + 1) * style.scale,
    height: (inkBottom - inkTop + 1) * style.scale,
    cellWidth: bitmap.width() * style.scale,
    cellHeight: bitmap.height() * style.scale,
    inkLeft,
    inkTop,
    inkRight,
    inkBottom,
  };
};

const getMeasuredWidth = (measure: MeasuredBitmapText) => measure.width;

const getMeasuredHeight = (measure: MeasuredBitmapText, verticalMetric: VerticalMetric) => (
  verticalMetric === "cell" ? measure.cellHeight : measure.height
);

const fitsInBox = (measure: MeasuredBitmapText, box: Box, verticalMetric: VerticalMetric) => (
  getMeasuredWidth(measure) <= box.width && getMeasuredHeight(measure, verticalMetric) <= box.height
);

const pickBitmapMeasure = (text: string, box: Box, weight: FontWeight, verticalMetric: VerticalMetric, styles = terminusStyles): MeasuredBitmapText => {
  const candidates = uniqueSortedStyles(styles).filter((style) => style.weight === weight);
  const fallbackCandidates = candidates.length > 0 ? candidates : [uniqueSortedStyles(terminusStyles).filter((style) => style.weight === weight).at(-1)!];
  let smallest: MeasuredBitmapText | null = null;

  for (const style of fallbackCandidates) {
    const measure = measureBitmapText(text, style);
    smallest = measure;
    if (fitsInBox(measure, box, verticalMetric)) {
      return measure;
    }
  }

  return smallest!;
};

const alignInBox = (box: Box, measure: MeasuredBitmapText, alignX: AlignX, alignY: AlignY) => {
  let x = box.x;
  if (alignX === "center") {
    x = box.x + Math.floor((box.width - measure.width) / 2);
  } else if (alignX === "right") {
    x = box.x + box.width - measure.width;
  }

  let y = box.y;
  if (alignY === "center") {
    y = box.y + Math.floor((box.height - measure.height) / 2);
  } else if (alignY === "bottom") {
    y = box.y + box.height - measure.height;
  }

  return {x, y};
};

const drawBitmapTextToSurface = (
  surface: Surface,
  bitmap: BdfBitmap,
  x: number,
  y: number,
  color: string,
  options: {scale?: number} = {},
) => {
  const scale = options.scale ?? 1;
  const rows = bitmap.todata(2) as number[][];
  const rgba = parseHexColor(color);

  for (let rowIndex = 0; rowIndex < rows.length; rowIndex += 1) {
    const row = rows[rowIndex];
    for (let columnIndex = 0; columnIndex < row.length; columnIndex += 1) {
      if (row[columnIndex] !== 0) {
        for (let offsetY = 0; offsetY < scale; offsetY += 1) {
          for (let offsetX = 0; offsetX < scale; offsetX += 1) {
            blendPixel(surface, x + (columnIndex * scale) + offsetX, y + (rowIndex * scale) + offsetY, rgba);
          }
        }
      }
    }
  }
};

const fitBitmapTextInBox = ({
  box,
  text,
  alignX,
  alignY,
  color,
  weight,
  verticalMetric = "ink",
}: {
  box: Box;
  text: string;
  alignX: AlignX;
  alignY: AlignY;
  color: string;
  weight: FontWeight;
  verticalMetric?: VerticalMetric;
}): FittedBitmapText => {
  const measure = pickBitmapMeasure(text, box, weight, verticalMetric);
  const verticalMeasure = verticalMetric === "cell"
    ? {...measure, height: measure.cellHeight}
    : measure;
  const {x, y: alignedY} = alignInBox(box, verticalMeasure, alignX, alignY);
  const y = verticalMetric === "cell"
    ? alignedY + (measure.inkTop * measure.style.scale)
    : alignedY;
  return {
    text,
    color,
    x,
    y,
    left: x,
    right: x + measure.width,
    top: y,
    bottom: y + measure.height,
    width: measure.width,
    height: measure.height,
    measure,
  };
};

const renderFittedBitmapText = (width: number, height: number, fitted: FittedBitmapText): RenderedStrip => {
  const surface = createSurface(width, height);
  drawBitmapTextToSurface(
    surface,
    fitted.measure.bitmap,
    fitted.x - (fitted.measure.inkLeft * fitted.measure.style.scale),
    fitted.y - (fitted.measure.inkTop * fitted.measure.style.scale),
    fitted.color,
    {scale: fitted.measure.style.scale},
  );
  return {
    surface,
    bounds: {
      left: fitted.left,
      right: fitted.right,
      top: fitted.top,
      bottom: fitted.bottom,
      width: fitted.width,
      height: fitted.height,
    },
  };
};

const fitBitmapTextPairInBox = ({
  box,
  leftText,
  rightText,
  minGap,
  alignY,
  color,
  weight,
  verticalMetric = "ink",
}: {
  box: Box;
  leftText: string;
  rightText: string;
  minGap: number;
  alignY: AlignY;
  color: string;
  weight: FontWeight;
  verticalMetric?: VerticalMetric;
}) => {
  const candidates = uniqueSortedStyles(terminusStyles).filter((style) => style.weight === weight);
  const fallbackCandidates = candidates.length > 0 ? candidates : [uniqueSortedStyles(terminusStyles).filter((style) => style.weight === weight).at(-1)!];
  let smallestFallback: {left: FittedBitmapText; right: FittedBitmapText} | null = null;

  for (const style of fallbackCandidates) {
    const leftMeasure = measureBitmapText(leftText, style);
    const rightMeasure = measureBitmapText(rightText, style);
    const rowHeight = Math.max(
      getMeasuredHeight(leftMeasure, verticalMetric),
      getMeasuredHeight(rightMeasure, verticalMetric),
    );
    if (rowHeight > box.height) {
      const leftBox = {...box, width: leftMeasure.width, height: rowHeight};
      const rightBox = {...box, x: box.x + box.width - rightMeasure.width, width: rightMeasure.width, height: rowHeight};
      smallestFallback = {
        left: fitBitmapTextInBox({box: leftBox, text: leftText, alignX: "left", alignY, color, weight, verticalMetric}),
        right: fitBitmapTextInBox({box: rightBox, text: rightText, alignX: "right", alignY, color, weight, verticalMetric}),
      };
      continue;
    }
    const gap = box.width - leftMeasure.width - rightMeasure.width;
    const leftBox = {x: box.x, y: box.y, width: leftMeasure.width, height: box.height};
    const rightBox = {x: box.x + box.width - rightMeasure.width, y: box.y, width: rightMeasure.width, height: box.height};
    const fitted = {
      left: fitBitmapTextInBox({box: leftBox, text: leftText, alignX: "left", alignY, color, weight, verticalMetric}),
      right: fitBitmapTextInBox({box: rightBox, text: rightText, alignX: "right", alignY, color, weight, verticalMetric}),
    };
    if (gap >= minGap) {
      return fitted;
    }
    smallestFallback = fitted;
  }

  return smallestFallback!;
};

const renderFooterStrip = (panelWidth: number, panelHeight: number, panelTime: Date) => {
  const dateText = `${panelTime.getFullYear()}-${twoDigits(panelTime.getMonth() + 1)}-${twoDigits(panelTime.getDate())}`;
  const timeText = `${twoDigits(panelTime.getHours())}:${twoDigits(panelTime.getMinutes())}:${twoDigits(panelTime.getSeconds())}`;
  const footerText = `${dateText} ${timeText}`;
  const fitted = fitBitmapTextInBox({
    box: {x: 0, y: 0, width: panelWidth, height: panelHeight},
    text: footerText,
    alignX: "center",
    alignY: "bottom",
    color: "#5f7a93",
    weight: "regular",
  });
  return renderFittedBitmapText(panelWidth, panelHeight, fitted);
};

const gapStrip = (height: number): Strip => ({
  height: Math.max(0, height),
  render: (width) => ({
    surface: createSurface(width, Math.max(0, height)),
    bounds: {left: 0, right: 0, top: 0, bottom: 0, width: 0, height: 0},
  }),
});

const flexibleGapStrip = (): Strip => ({
  height: 0,
  flexible: true,
  render: (width) => ({
    surface: createSurface(width, 0),
    bounds: {left: 0, right: 0, top: 0, bottom: 0, width: 0, height: 0},
  }),
});

const textStrip = (height: number, render: (width: number, height: number) => FittedBitmapText): Strip => ({
  height,
  render: (width) => renderFittedBitmapText(width, height, render(width, height)),
});

const pairStrip = (height: number, render: (width: number, height: number) => {left: FittedBitmapText; right: FittedBitmapText}): Strip => ({
  height,
  render: (width) => {
    const surface = createSurface(width, height);
    const fitted = render(width, height);
    const left = renderFittedBitmapText(width, height, fitted.left);
    const right = renderFittedBitmapText(width, height, fitted.right);
    blitSurface(surface, left.surface, 0, 0);
    blitSurface(surface, right.surface, 0, 0);
    return {
      surface,
      bounds: {
        left: Math.min(left.bounds.left, right.bounds.left),
        right: Math.max(left.bounds.right, right.bounds.right),
        top: Math.min(left.bounds.top, right.bounds.top),
        bottom: Math.max(left.bounds.bottom, right.bounds.bottom),
        width: Math.max(left.bounds.right, right.bounds.right) - Math.min(left.bounds.left, right.bounds.left),
        height: Math.max(left.bounds.bottom, right.bounds.bottom) - Math.min(left.bounds.top, right.bounds.top),
      },
    };
  },
});

const composeStrips = (width: number, height: number, strips: readonly Strip[]) => {
  const flexibleCount = strips.filter((strip) => strip.flexible).length;
  const fixedHeight = strips.reduce((sum, strip) => sum + (strip.flexible ? 0 : strip.height), 0);
  const remainingHeight = height - fixedHeight;

  if (remainingHeight < 0) {
    throw new Error(`Strip layout overflow: expected ${height}, fixed content uses ${fixedHeight}`);
  }

  if (flexibleCount === 0 && fixedHeight !== height) {
    throw new Error(`Strip layout height mismatch: expected ${height}, got ${fixedHeight}`);
  }

  if (flexibleCount > 0 && remainingHeight % flexibleCount !== 0) {
    throw new Error(`Strip layout flex mismatch: remaining ${remainingHeight} cannot be evenly divided across ${flexibleCount} flexible gaps`);
  }

  const flexibleHeight = flexibleCount > 0 ? remainingHeight / flexibleCount : 0;
  const resolvedStrips = strips.map((strip) => strip.flexible ? {
    ...strip,
    height: flexibleHeight,
    render: (stripWidth: number) => ({
      surface: createSurface(stripWidth, flexibleHeight),
      bounds: {left: 0, right: 0, top: 0, bottom: 0, width: 0, height: 0},
    }),
  } : strip);
  const totalHeight = resolvedStrips.reduce((sum, strip) => sum + strip.height, 0);
  if (totalHeight !== height) {
    throw new Error(`Strip layout height mismatch: expected ${height}, got ${totalHeight}`);
  }

  const surface = createSurface(width, totalHeight);
  let y = 0;
  for (const strip of resolvedStrips) {
    const rendered = strip.render(width);
    blitSurface(surface, rendered.surface, 0, y);
    y += strip.height;
  }
  return surface;
};

export const buildInfoPanelRgb565 = (panelWidth: number, panelHeight: number) => (panelTime: Date, row: InfoPanelRow | null) => {
  const view = getInfoPanelView(panelTime, row);
  const footerHeight = 14;
  const socColor = view.soc === undefined
    ? "#ffd8dd"
    : view.soc < 30 ? "#ff4d4d" : view.soc < 50 ? "#ff8c3a" : view.soc < 80 ? "#ffd74d" : "#52db76";
  const chargingColor = view.chargingWattText === "-- W"
    ? "#ffd8dd"
    : view.chargingWatt !== undefined && view.chargingWatt >= 0 ? "#7ef5b0" : "#ffd27f";
  const surface = composeStrips(panelWidth, panelHeight, [
    gapStrip(2),
    textStrip(40, (width, height) => fitBitmapTextInBox({
      box: {x: 0, y: 0, width, height},
      text: view.socText,
      alignX: "center",
      alignY: "top",
      color: socColor,
      weight: "bold",
    })),
    gapStrip(6),
    textStrip(16, (width, height) => fitBitmapTextInBox({
      box: {x: 4, y: 0, width: width - 8, height},
      text: view.chargingWattText,
      alignX: "center",
      alignY: "top",
      color: chargingColor,
      weight: "bold",
    })),
    gapStrip(6),
    textStrip(12, (width, height) => fitBitmapTextInBox({
      box: {x: 4, y: 0, width: width - 8, height},
      text: view.temperatureText,
      alignX: "center",
      alignY: "center",
      color: "#f7b877",
      weight: "regular",
    })),
    gapStrip(6),
    textStrip(16, (width, height) => fitBitmapTextInBox({
      box: {x: 6, y: 0, width: width - 12, height},
      text: view.pvSumWattText,
      alignX: "center",
      alignY: "center",
      color: "#82d4ff",
      weight: "bold",
    })),
    gapStrip(6),
    pairStrip(10, (width, height) => fitBitmapTextPairInBox({
      box: {x: 6, y: 0, width: width - 12, height},
      leftText: view.pvEnergyDayText,
      rightText: view.pvEnergyMonthText,
      minGap: 8,
      alignY: "center",
      color: "#82d4ff",
      weight: "regular",
      verticalMetric: "cell",
    })),
    gapStrip(2),
    pairStrip(10, (width, height) => fitBitmapTextPairInBox({
      box: {x: 6, y: 0, width: width - 12, height},
      leftText: view.pvEnergyWeekText,
      rightText: view.pvEnergyYearText,
      minGap: 8,
      alignY: "center",
      color: "#82d4ff",
      weight: "regular",
      verticalMetric: "cell",
    })),
    flexibleGapStrip(),
    {
      height: footerHeight,
      render: () => renderFooterStrip(panelWidth, footerHeight, view.footerTime),
    },
  ]);

  return toRgb565(surface);
};

export const rgb565ToPng = (rgb565: Buffer, width: number, height: number) => {
  const canvas = createCanvas(width, height);
  const ctx = canvas.getContext('2d');
  const imageData = ctx.createImageData(width, height);

  for (let i = 0, j = 0; i < rgb565.length; i += 2, j += 4) {
    const c = (rgb565[i] << 8) | rgb565[i + 1];
    const r5 = (c >> 11) & 0x1f;
    const g6 = (c >> 5) & 0x3f;
    const b5 = c & 0x1f;

    imageData.data[j] = (r5 << 3) | (r5 >> 2);
    imageData.data[j + 1] = (g6 << 2) | (g6 >> 4);
    imageData.data[j + 2] = (b5 << 3) | (b5 >> 2);
    imageData.data[j + 3] = 255;
  }

  ctx.putImageData(imageData, 0, 0);
  return canvas.toBuffer('image/png');
};
