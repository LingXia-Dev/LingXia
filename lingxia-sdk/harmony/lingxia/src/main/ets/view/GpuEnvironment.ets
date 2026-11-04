import { display, window } from '@kit.ArkUI';
import { i18n } from '@kit.LocalizationKit';
import { common, Configuration } from '@kit.AbilityKit';

/**
 * The OS facts a GPU page's `env` is derived from, in device pixels, with the keys
 * `gpuPageMeasured` reads. Insets and keyboard are relative to the page's own rect.
 */
export class GpuFacts {
  width: number = 0;
  height: number = 0;
  scale: number = 1;
  dark: boolean = false;
  /** Status bar, cutout and navigation indicator over the page: top, right, bottom, left. */
  system_insets: number[] = [0, 0, 0, 0];
  /** Host navigation and tab bars over the page, same order. */
  chrome_insets: number[] = [0, 0, 0, 0];
  keyboard: number = 0;
  smallest_width: number = 0;
  locale: string = '';
  fine_pointer: boolean = false;
  animation_scale: number = 1;
  reduce_motion: boolean = false;
  high_contrast: boolean = false;
  text_scale: number[][] | null = null;
  hour_cycle: string = 'h23';
  first_weekday: number = 1;
}

/** A page rect on screen, device pixels. */
export class GpuRect {
  x: number = 0;
  y: number = 0;
  width: number = 0;
  height: number = 0;
}

function overlap(area: window.AvoidArea, page: GpuRect): number[] {
  const top = Math.max(0, area.topRect.top + area.topRect.height - page.y);
  const bottom = Math.max(0, page.y + page.height - area.bottomRect.top);
  const left = Math.max(0, area.leftRect.left + area.leftRect.width - page.x);
  const right = Math.max(0, page.x + page.width - area.rightRect.left);
  return [
    area.topRect.height > 0 ? top : 0,
    area.rightRect.width > 0 ? right : 0,
    area.bottomRect.height > 0 ? bottom : 0,
    area.leftRect.width > 0 ? left : 0,
  ];
}

function larger(a: number[], b: number[]): number[] {
  return [Math.max(a[0], b[0]), Math.max(a[1], b[1]), Math.max(a[2], b[2]), Math.max(a[3], b[3])];
}

/** Samples the facts for a page at `rect` in `win`, with the host bars covering `chrome`. */
export function sampleFacts(context: common.UIAbilityContext, win: window.Window, rect: GpuRect,
  chrome: number[], locale: string): GpuFacts {
  const facts = new GpuFacts();
  const screen = display.getDefaultDisplaySync();
  facts.width = Math.round(rect.width);
  facts.height = Math.round(rect.height);
  facts.scale = screen.densityPixels;
  facts.smallest_width = Math.min(screen.width, screen.height) / screen.densityPixels;
  const config: Configuration = context.config;
  facts.dark = config.colorMode === 0;
  facts.locale = locale;
  const system = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
  const cutout = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_CUTOUT);
  const indicator = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_NAVIGATION_INDICATOR);
  facts.system_insets = larger(larger(overlap(system, rect), overlap(cutout, rect)), overlap(indicator, rect));
  facts.chrome_insets = chrome;
  facts.keyboard = overlap(win.getWindowAvoidArea(window.AvoidAreaType.TYPE_KEYBOARD), rect)[2];
  // HarmonyOS scales text linearly up to its configured maximum.
  const scale = config.fontSizeScale ?? 1;
  facts.text_scale = scale === 1 ? null : [[0, 0], [16384, 16384 * scale]];
  facts.hour_cycle = i18n.System.is24HourClock() ? 'h23' : 'h12';
  facts.first_weekday = i18n.getCalendar(locale).getFirstDayOfWeek();
  return facts;
}
