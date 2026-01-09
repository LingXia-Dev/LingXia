/**
 * Native component rendering types for HarmonyOS
 * Uses official ArkWeb same-level rendering pattern
 */

import { hilog } from '@kit.PerformanceAnalysisKit';

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

  forEach(callback: (key: string, value: NativeComponentValue) => void): void {
    this.data.forEach((value, key) => {
      callback(key, value);
    });
  }

  toJSONString(): string {
    const parts: string[] = [];
    this.data.forEach((value, key) => {
      const jsonValue = JSON.stringify(value);
      if (jsonValue === undefined) {
        return;
      }
      parts.push(`${JSON.stringify(key)}:${jsonValue}`);
    });
    return `{${parts.join(',')}}`;
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
  hilog.error(DOMAIN, TAG, message);
}
