/**
 * BuilderUtils - Shared utility functions for native component builders
 */
import { NativeComponentRecord } from '../NativeComponentTypes';
import { window } from '@kit.ArkUI';
import { common } from '@kit.AbilityKit';

export const DEFAULT_PLACEHOLDER_COLOR: string = '#8A8A8A';

export function getStringProp(props: NativeComponentRecord | undefined | null, key: string, defaultValue: string = ''): string {
  if (!props) {
    return defaultValue;
  }
  const value = props.get(key);
  return (typeof value === 'string') ? value : defaultValue;
}

export function getBooleanProp(props: NativeComponentRecord | undefined | null, key: string, defaultValue: boolean = false): boolean {
  if (!props) {
    return defaultValue;
  }
  const value = props.get(key);
  if (typeof value === 'boolean') {
    return value;
  }
  if (typeof value === 'number') {
    return value !== 0;
  }
  if (typeof value === 'string') {
    const normalized = value.trim().toLowerCase();
    if (normalized === 'true' || normalized === '1') return true;
    if (normalized === 'false' || normalized === '0') return false;
  }
  return defaultValue;
}

export function getNumberProp(props: NativeComponentRecord | undefined | null, key: string, defaultValue: number = 0): number {
  if (!props) {
    return defaultValue;
  }
  const value = props.get(key);
  if (typeof value === 'number') {
    return value;
  }
  if (typeof value === 'string') {
    const parsed = Number(value);
    if (!Number.isNaN(parsed)) {
      return parsed;
    }
  }
  return defaultValue;
}

export function parsePlaceholderColor(styleValue: string): string {
  const normalized = styleValue.trim();
  if (!normalized) {
    return DEFAULT_PLACEHOLDER_COLOR;
  }
  const styleMatch = normalized.match(/color\s*:\s*([^;]+)/i);
  const color = (styleMatch && styleMatch.length > 1) ? styleMatch[1].trim() : normalized;
  return color.length > 0 ? color : DEFAULT_PLACEHOLDER_COLOR;
}

export function resolvePlaceholderColor(props: NativeComponentRecord | undefined | null): string {
  const explicit = getStringProp(
    props,
    'placeholderColor',
    getStringProp(props, 'placeholder-color', '')
  );
  if (explicit.trim().length > 0) {
    return parsePlaceholderColor(explicit);
  }
  const style = getStringProp(
    props,
    'placeholderStyle',
    getStringProp(props, 'placeholder-style', '')
  );
  return parsePlaceholderColor(style);
}

export async function readKeyboardHeightVp(uiContext: UIContext): Promise<number> {
  try {
    const context = uiContext.getHostContext() as common.UIAbilityContext;
    if (!context) {
      return 0;
    }
    const mainWindow = await window.getLastWindow(context);
    const avoidArea = await mainWindow.getWindowAvoidArea(window.AvoidAreaType.TYPE_KEYBOARD);
    const bottomHeightPx = avoidArea.bottomRect?.height ?? 0;
    const converted = Number(px2vp(bottomHeightPx));
    if (!Number.isFinite(converted)) {
      return 0;
    }
    return Math.max(0, converted);
  } catch (_error) {
    return 0;
  }
}
