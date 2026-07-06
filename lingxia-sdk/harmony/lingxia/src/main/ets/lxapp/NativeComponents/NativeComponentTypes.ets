/**
 * Native component rendering types for HarmonyOS
 * Uses official ArkWeb same-level rendering pattern
 */

import { hilog } from '@kit.PerformanceAnalysisKit';
import { LxLog } from '../../app/LxLog';

const DOMAIN = 0x0000;
const TAG = 'LingXia.NativeComponent';

export type NativeComponentValue = string | number | boolean | null | undefined | object;

interface NativeComponentGettableObject {
  get?: (key: string) => NativeComponentValue;
}

export class NativeComponentRecord {
  private data: Map<string, NativeComponentValue> = new Map();

  set(key: string, value: NativeComponentValue): void {
    this.data.set(key, value);
  }

  get(key: string): NativeComponentValue | undefined {
    return this.data.get(key);
  }

  has(key: string): boolean {
    return this.data.has(key);
  }

  delete(key: string): void {
    this.data.delete(key);
  }

  forEach(callback: (key: string, value: NativeComponentValue) => void): void {
    this.data.forEach((value, key) => {
      callback(key, value);
    });
  }

  private normalizeValue(value: NativeComponentValue): NativeComponentValue {
    if (value instanceof NativeComponentRecord) {
      return value.toPlainObject();
    }
    if (Array.isArray(value)) {
      const normalizedArray: NativeComponentValue[] = [];
      (value as NativeComponentValue[]).forEach((item: NativeComponentValue) => {
        normalizedArray.push(this.normalizeValue(item));
      });
      return normalizedArray;
    }
    if (value !== null && value !== undefined && typeof value === 'object') {
      const source = value as Record<string, NativeComponentValue>;
      const normalizedObject: Record<string, NativeComponentValue> = {};
      Object.keys(source).forEach((key: string) => {
        normalizedObject[key] = this.normalizeValue(source[key]);
      });
      return normalizedObject;
    }
    return value;
  }

  toPlainObject(): Record<string, NativeComponentValue> {
    const out: Record<string, NativeComponentValue> = {};
    this.data.forEach((value: NativeComponentValue, key: string) => {
      out[key] = this.normalizeValue(value);
    });
    return out;
  }

  toJSONString(): string {
    return JSON.stringify(this.toPlainObject());
  }

  static fromESObject(obj: ESObject): NativeComponentRecord {
    const record = new NativeComponentRecord();
    const gettable = obj as NativeComponentGettableObject;
    const plainObj = obj as Record<string, NativeComponentValue>;

    Object.keys(plainObj).forEach((key: string) => {
      let value: NativeComponentValue;

      // Try to get value using get() method if it exists (for ESObject)
      if (typeof gettable.get === 'function') {
        value = gettable.get(key) as NativeComponentValue;
      } else {
        // Fallback to direct property access (for plain objects)
        value = plainObj[key];
      }

      record.set(key, value);
    });
    return record;
  }
}

export class NativeComponentEmptyDetail {}

/**
 * Log helper
 */
export function nativeComponentLog(message: string): void {
  hilog.info(DOMAIN, TAG, message);
}

export function nativeComponentError(message: string): void {
  LxLog.e(TAG, message);
}
