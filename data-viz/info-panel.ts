import {createRequire} from "node:module";
import {createCanvas} from "@napi-rs/canvas";
import type {Canvas, SKRSContext2D} from "@napi-rs/canvas";
import {statements, database} from "./queries.ts";
import * as fontkit from "fontkit";

const require = createRequire(import.meta.url);

const robotoRegularPath = require.resolve("roboto-font/fonts/Roboto/roboto-regular-webfont.ttf");
const robotoBoldPath = require.resolve("roboto-font/fonts/Roboto/roboto-bold-webfont.ttf");
const robotoRegularFont = fontkit.openSync(robotoRegularPath);
const robotoBoldFont = fontkit.openSync(robotoBoldPath);

type InfoPanelRow = {
  state_of_charge: number | null;
  state_of_charge_timestamp: number | null;
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

const twoDigits = (n: number) => String(n).padStart(2, "0");

type TextBaseline = "alphabetic" | "top" | "middle" | "bottom" | "hanging" | "ideographic";
type FontWeight = "regular" | "bold";

type GlyphPosition = {
  xAdvance: number;
  yAdvance: number;
  xOffset: number;
  yOffset: number;
};

type Glyph = {
  bbox: {
    minX: number;
    minY: number;
    maxX: number;
    maxY: number;
  };
  path: {
    toFunction: () => (ctx: SKRSContext2D) => void;
  };
};

type GlyphRun = {
  glyphs: Glyph[];
  positions: GlyphPosition[];
};

type MeasuredPathText = {
  run: GlyphRun;
  scale: number;
  width: number;
  minX: number;
  ascent: number;
  descent: number;
};

const getFontForWeight = (weight: FontWeight) => {
  if (weight === "regular") {
    return robotoRegularFont;
  }
  return robotoBoldFont;
};

const measurePathText = (text: string, size: number, weight: FontWeight): MeasuredPathText => {
  const font = getFontForWeight(weight);
  const run = font.layout(text) as GlyphRun;
  const scale = size / font.unitsPerEm;

  let penX = 0;
  let penY = 0;
  let minX = 0;
  let maxX = 0;
  let minY = 0;
  let maxY = 0;

  for (let i = 0; i < run.glyphs.length; i += 1) {
    const glyph = run.glyphs[i];
    const pos = run.positions[i];
    const xOffset = pos?.xOffset ?? 0;
    const yOffset = pos?.yOffset ?? 0;
    const xAdvance = pos?.xAdvance ?? 0;
    const yAdvance = pos?.yAdvance ?? 0;
    const left = penX + xOffset + glyph.bbox.minX;
    const right = penX + xOffset + glyph.bbox.maxX;
    const bottom = penY + yOffset + glyph.bbox.minY;
    const top = penY + yOffset + glyph.bbox.maxY;
    minX = Math.min(minX, left);
    maxX = Math.max(maxX, right);
    minY = Math.min(minY, bottom);
    maxY = Math.max(maxY, top);
    penX += xAdvance;
    penY += yAdvance;
  }

  maxX = Math.max(maxX, penX);

  const width = (maxX - minX) * scale;
  const ascent = maxY * scale;
  const descent = Math.max(0, -minY * scale);

  return {
    run,
    scale,
    width,
    minX,
    ascent,
    descent,
  };
};

const resolveBaselineY = (y: number, baseline: TextBaseline, ascent: number, descent: number) => {
  if (baseline === "top" || baseline === "hanging") {
    return y + ascent;
  }
  if (baseline === "middle") {
    return y + ((ascent - descent) / 2);
  }
  if (baseline === "bottom" || baseline === "ideographic") {
    return y - descent;
  }
  return y;
};

const drawPathText = (
  ctx: SKRSContext2D,
  measured: MeasuredPathText,
  x: number,
  baselineY: number,
  color: string,
) => {
  const originX = x - (measured.minX * measured.scale);

  ctx.save();
  ctx.fillStyle = color;

  let penX = 0;
  let penY = 0;
  for (let i = 0; i < measured.run.glyphs.length; i += 1) {
    const glyph = measured.run.glyphs[i];
    const pos = measured.run.positions[i];
    const xOffset = pos?.xOffset ?? 0;
    const yOffset = pos?.yOffset ?? 0;
    const xAdvance = pos?.xAdvance ?? 0;
    const yAdvance = pos?.yAdvance ?? 0;

    const glyphX = originX + ((penX + xOffset) * measured.scale);
    const glyphY = baselineY - ((penY + yOffset) * measured.scale);
    ctx.save();
    ctx.translate(glyphX, glyphY);
    ctx.scale(measured.scale, -measured.scale);
    ctx.beginPath();
    glyph.path.toFunction()(ctx);
    ctx.fillStyle = color;
    ctx.fill();
    ctx.restore();

    penX += xAdvance;
    penY += yAdvance;
  }
  ctx.restore();
};

const drawCenteredFitText = (
  ctx: SKRSContext2D,
  text: string,
  y: number,
  maxWidth: number,
  color: string,
  startSize: number,
  options: {baseline?: TextBaseline; weight?: FontWeight} = {},
) => {
  const baseline = options.baseline ?? "alphabetic";
  const weight = options.weight ?? "bold";

  let size = startSize;
  let measured = measurePathText(text, size, weight);
  for (; size >= 2; size -= 2) {
    const candidate = measurePathText(text, size, weight);
    measured = candidate;
    if (candidate.width <= maxWidth) {
      break;
    }
  }

  const baselineY = resolveBaselineY(y, baseline, measured.ascent, measured.descent);
  const top = baselineY - measured.ascent;
  const bottom = baselineY + measured.descent;
  const x = Math.max(0, Math.floor((ctx.canvas.width - measured.width) / 2));

  drawPathText(ctx, measured, x, baselineY, color);

  return {
    x,
    y: baselineY,
    width: measured.width,
    height: measured.ascent + measured.descent,
    top,
    bottom,
    fontSize: size,
    baseline,
  };
};

const drawDateTimeFooter = (panelWidth: number, panelHeight: number) => (ctx: SKRSContext2D, panelTime: Date) => {
  const dateText = `${panelTime.getFullYear()}-${twoDigits(panelTime.getMonth() + 1)}-${twoDigits(panelTime.getDate())}`;
  const timeText = `${twoDigits(panelTime.getHours())}:${twoDigits(panelTime.getMinutes())}:${twoDigits(panelTime.getSeconds())}`;
  const footerText = `${dateText} ${timeText}`;

  drawCenteredFitText(ctx, footerText, panelHeight, panelWidth - 8, "#5f7a93", 10, {baseline: "bottom"});
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

const toRgb565 = (canvas: Canvas) => {
  const ctx = canvas.getContext("2d");
  const rgba = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
  const rgb565 = Buffer.alloc(canvas.width * canvas.height * 2);

  for (let i = 0, j = 0; i < rgba.length; i += 4, j += 2) {
    const r = rgba[i];
    const g = rgba[i + 1];
    const b = rgba[i + 2];
    const c = ((r & 0xf8) << 8) | ((g & 0xfc) << 3) | (b >> 3);
    rgb565[j] = (c >> 8) & 0xff;
    rgb565[j + 1] = c & 0xff;
  }

  return rgb565;
};

export const buildInfoPanelRgb565 = (panelWidth: number, panelHeight: number) => (panelTime: Date, row: InfoPanelRow | null) => {
  const canvas = createCanvas(panelWidth, panelHeight);
  const ctx = canvas.getContext("2d");

  ctx.fillStyle = "#000000";
  ctx.fillRect(0, 0, panelWidth, panelHeight);

  const hasData = row !== null && Object.values(row).every((value) => value !== null);
  let footerTime = panelTime;
  if (hasData) {
    const latestTimestamp = Math.max(
      ...Object.entries(row)
        .filter(([key, value]) => key.endsWith("_timestamp") && typeof value === "number")
        .map(([, value]) => value),
    );
    footerTime = new Date(latestTimestamp);
  }

  if (!hasData) {
    const hibaMeasured = measurePathText("HIBA", 18, "bold");
    const hibaBaselineY = resolveBaselineY(72, "alphabetic", hibaMeasured.ascent, hibaMeasured.descent);
    drawPathText(ctx, hibaMeasured, 12, hibaBaselineY, "#ffd8dd");

    const noDataMeasured = measurePathText("Nincs adat", 12, "regular");
    const noDataBaselineY = resolveBaselineY(92, "alphabetic", noDataMeasured.ascent, noDataMeasured.descent);
    drawPathText(ctx, noDataMeasured, 12, noDataBaselineY, "#ffd8dd");

    drawDateTimeFooter(panelWidth, panelHeight)(ctx, footerTime);
    return toRgb565(canvas);
  }

  const soc = Math.round(row.state_of_charge!);
  const chargingWatt = Math.round(row.battery_charging_watt!);
  const pvEnergyDayKwh = row.pv_energy_day_wh! / 1000;
  const pvEnergyWeekKwh = row.pv_energy_week_wh! / 1000;
  const pvEnergyMonthKwh = row.pv_energy_month_wh! / 1000;
  const pvEnergyYearKwh = row.pv_energy_year_wh! / 1000;
  const temperatures = [
    row.mos_temperature!,
    row.battery_temperature_1!,
    row.battery_temperature_2!,
    row.battery_temperature_3!,
    row.battery_temperature_4!,
    row.battery_temperature_5!,
  ];
  const minTemperature = Math.min(...temperatures);
  const maxTemperature = Math.max(...temperatures);

  const socColor = soc < 30 ? "#ff4d4d" : soc < 50 ? "#ff8c3a" : soc < 80 ? "#ffd74d" : "#52db76";
  const socText = `${soc}%`;
  const socDrawn = drawCenteredFitText(ctx, socText, 10, panelWidth - 10, socColor, 40, {baseline: "top"});

  const chargingColor = chargingWatt >= 0 ? "#7ef5b0" : "#ffd27f";
  const chargingText = `${chargingWatt} W`;
  const chargingDrawn = drawCenteredFitText(ctx, chargingText, socDrawn.bottom + 10, panelWidth - 10, chargingColor, 20, {baseline: "top"});

  const temperatureText = `${minTemperature.toFixed(1)} °C - ${maxTemperature.toFixed(1)} °C`;
  const temperatureDrawn = drawCenteredFitText(ctx, temperatureText, chargingDrawn.bottom + 20, panelWidth - 10, "#f7b877", 12, {baseline: "top"});

  const pvEnergyText = `N ${pvEnergyDayKwh.toFixed(1)} H ${pvEnergyMonthKwh.toFixed(1)} kWh`;
  const pvEnergyDrawn = drawCenteredFitText(ctx, pvEnergyText, temperatureDrawn.bottom + 14, panelWidth - 10, "#82d4ff", 12, {baseline: "top"});

  const pvEnergyLongText = `He ${pvEnergyWeekKwh.toFixed(1)} É ${pvEnergyYearKwh.toFixed(1)} kWh`;
  drawCenteredFitText(ctx, pvEnergyLongText, pvEnergyDrawn.bottom + 4, panelWidth - 10, "#82d4ff", 12, {baseline: "top"});
  drawDateTimeFooter(panelWidth, panelHeight)(ctx, footerTime);

  return toRgb565(canvas);
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
