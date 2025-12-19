/**
 * Same-level rendering types for HarmonyOS
 * Uses official ArkWeb same-level rendering pattern
 */

import { hilog } from '@kit.PerformanceAnalysisKit';

const DOMAIN = 0x0000;
const TAG = 'LingXia.SameLevel';

export type SameLevelValue = string | number | boolean | null | undefined | object;

export class SameLevelRecord {
  private data: Map<string, SameLevelValue> = new Map();

  set(key: string, value: SameLevelValue): void {
    this.data.set(key, value);
  }

  get(key: string): SameLevelValue | undefined {
    return this.data.get(key);
  }

  forEach(callback: (key: string, value: SameLevelValue) => void): void {
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

  static fromESObject(obj: ESObject): SameLevelRecord {
    const record = new SameLevelRecord();
    Object.keys(obj as object).forEach((key: string) => {
      record.set(key, obj.get(key) as SameLevelValue);
    });
    return record;
  }
}

/**
 * Log helper
 */
export function sameLevelLog(message: string): void {
  hilog.info(DOMAIN, TAG, message);
}

export function sameLevelError(message: string): void {
  hilog.error(DOMAIN, TAG, message);
}
