import { hilog } from '@kit.PerformanceAnalysisKit';
import { scanBarcode, scanCore } from '@kit.ScanKit';
import { LxApp } from '../LxApp';
import { onCallback } from 'liblingxia.so';
import { BusinessError } from '@kit.BasicServicesKit';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Scan';

interface ScanCodePayload {
  // Accept both scanType (string or list) and scanTypes (list) for compatibility
  scanType?: string | string[];
  scanTypes?: string[];
  onlyFromCamera: boolean;
  callbackId: string | number;
}

interface ScanReply {
  scanResult: string;
  scanType: string;
}

export class LxAppScan {
  public static scanCode(payload: string): boolean {
    try {
      const options: ScanCodePayload = JSON.parse(payload || '{}');

      const hasCb = options && (typeof options.callbackId === 'string' || typeof options.callbackId === 'number');
      if (!hasCb) {
        hilog.error(DOMAIN, TAG, 'scanCode: invalid payload');
        return false;
      }

      const normalized: ScanCodePayload = {
        callbackId: String(options.callbackId),
        // Normalize both scanType and scanTypes into scanTypes list; empty => ALL
        scanType: options.scanType,
        scanTypes: Array.isArray(options.scanTypes) ? options.scanTypes : undefined,
        onlyFromCamera: options.onlyFromCamera !== false
      };

      LxAppScan.startScan(normalized);
      return true;
    } catch (err) {
      const message = err instanceof Error ? err.message : `${err}`;
      hilog.error(DOMAIN, TAG, `scanCode: failed to parse payload: ${message}`);
      return false;
    }
  }

  private static async startScan(options: ScanCodePayload): Promise<void> {
    try {
      if (!LxApp.context) {
        onCallback(String(options.callbackId), false, '1000');
        return;
      }

      // Configure scan options based on the requested scan types
      const scanTypes: scanCore.ScanType[] = LxAppScan.convertScanTypes(options);

      const scanOptions: scanBarcode.ScanOptions = {
        scanTypes: scanTypes,
        enableMultiMode: false,
        enableAlbum: !options.onlyFromCamera
      };

      // Start scan and normalize to a single result without using 'any' or index access
      interface ScanResultLike { originalValue?: string; value?: string; text?: string; scanType?: scanCore.ScanType; }
      const rawRes: scanBarcode.ScanResult | scanBarcode.ScanResult[] = await scanBarcode.startScanForResult(LxApp.context, scanOptions);
      let res: ScanResultLike | undefined;
      if (Array.isArray(rawRes)) {
        for (const item of (rawRes as Array<ScanResultLike>)) { res = item; break; }
      } else {
        res = rawRes as ScanResultLike;
      }
      const text: string = (res?.originalValue ?? res?.value ?? res?.text) ?? '';
      const mappedType: string = LxAppScan.getScanTypeString((res?.scanType as scanCore.ScanType) ?? scanCore.ScanType.ALL);
      const typeOk: boolean = (res?.scanType !== undefined) ? LxAppScan.isAllowedType(res!.scanType as scanCore.ScanType, scanTypes) : false;

      if (text && typeOk) {
        const payload: ScanReply = {
          scanResult: text,
          scanType: LxAppScan.getScanTypeString(res!.scanType as scanCore.ScanType),
        };
        const out = JSON.stringify(payload);
        onCallback(String(options.callbackId), true, out);
      } else {
        onCallback(String(options.callbackId), false, '1001');
      }
    } catch (err) {
      const error = err as BusinessError;
      // Handle cancel case
      if (error.code === scanCore.ScanErrorCode.SCAN_SERVICE_CANCELED) {
        onCallback(String(options.callbackId), false, '2000');
      } else {
        const message = error.message || (err instanceof Error ? err.message : `${err}`);
        hilog.error(DOMAIN, TAG, `startScan failed: ${message}`);
        onCallback(String(options.callbackId), false, '1001');
      }
    }
  }

  private static convertScanTypes(options: ScanCodePayload): scanCore.ScanType[] {
    const tokens: string[] = [];
    const pushToken = (v: string): void => {
      const s = (v || '').trim();
      if (s) tokens.push(s);
    };
    if (Array.isArray(options.scanTypes)) {
      for (const it of options.scanTypes) { pushToken(String(it)); }
    }
    if (options.scanType !== undefined) {
      if (Array.isArray(options.scanType)) {
        for (const it of options.scanType) { pushToken(String(it)); }
      } else {
        // Allow comma/space separated, or a single token
        String(options.scanType)
          .split(/[,\s]+/)
          .forEach((it: string): void => { pushToken(it); });
      }
    }

    const result: scanCore.ScanType[] = [];
    const toKey = (raw: string): string => raw.toLowerCase().replace(/[_\-\s]/g, '');

    for (const raw of tokens) {
      const key = toKey(raw);
      if (key === 'all' || key === 'auto') {
        // Only include types we support; do NOT use ScanType.ALL to avoid unsupported types sneaking in
        result.push(scanCore.ScanType.QR_CODE);
        result.push(scanCore.ScanType.DATAMATRIX_CODE);
        result.push(scanCore.ScanType.PDF417_CODE);
        result.push(scanCore.ScanType.ONE_D_CODE);
      } else if (key === 'qrcode' || key === 'qr' || key === 'qrcodecode') {
        result.push(scanCore.ScanType.QR_CODE);
      } else if (key === 'datamatrix' || key === 'datamatrixcode') {
        // Use correct enum name per ScanKit
        result.push(scanCore.ScanType.DATAMATRIX_CODE);
      } else if (key === 'pdf417' || key === 'pdf417code') {
        result.push(scanCore.ScanType.PDF417_CODE);
      } else if (key === 'barcode' || key === 'bar' || key === 'barcode1d' || key === 'barcode_1d' || key === '1d' || key === 'oned') {
        // Restrict to 1D barcodes
        result.push(scanCore.ScanType.ONE_D_CODE);
      }
    }

    // If nothing matched, default to ALL
    if (result.length === 0) {
      return [scanCore.ScanType.ALL];
    }
    // Deduplicate while preserving order
    const unique: scanCore.ScanType[] = [];
    const seen = new Set<number>();
    for (const item of result) {
      const v: number = item as number;
      if (!seen.has(v)) {
        unique.push(item);
        seen.add(v);
      }
    }
    return unique;
  }

  private static isOneDType(t: scanCore.ScanType): boolean {
    switch (t) {
      case scanCore.ScanType.CODABAR_CODE:
      case scanCore.ScanType.CODE39_CODE:
      case scanCore.ScanType.CODE93_CODE:
      case scanCore.ScanType.CODE128_CODE:
      case scanCore.ScanType.EAN8_CODE:
      case scanCore.ScanType.EAN13_CODE:
      case scanCore.ScanType.ITF14_CODE:
      case scanCore.ScanType.UPC_A_CODE:
      case scanCore.ScanType.UPC_E_CODE:
      case scanCore.ScanType.ONE_D_CODE:
        return true;
      default:
        return false;
    }
  }

  // Ensure the returned type is within requested whitelist
  private static isAllowedType(actual: scanCore.ScanType, allowList: scanCore.ScanType[]): boolean {
    // Allow everything when ALL is present or allowList is empty
    if (!allowList || allowList.length === 0 || allowList.indexOf(scanCore.ScanType.ALL) !== -1) {
      return true;
    }
    // Exact match allowed
    if (allowList.indexOf(actual) !== -1) {
      return true;
    }
    // ONE_D_CODE in allowList allows any 1D subtype
    if (allowList.indexOf(scanCore.ScanType.ONE_D_CODE) !== -1 && LxAppScan.isOneDType(actual)) {
      return true;
    }
    return false;
  }

  private static getScanTypeString(type: scanCore.ScanType): string {
    switch (type) {
      case scanCore.ScanType.QR_CODE:
        return 'QR_CODE';
      case scanCore.ScanType.DATAMATRIX_CODE:
        return 'DATA_MATRIX';
      case scanCore.ScanType.PDF417_CODE:
        return 'PDF_417';
      case scanCore.ScanType.CODABAR_CODE:
        return 'CODABAR';
      case scanCore.ScanType.CODE39_CODE:
        return 'CODE_39';
      case scanCore.ScanType.CODE93_CODE:
        return 'CODE_93';
      case scanCore.ScanType.CODE128_CODE:
        return 'CODE_128';
      case scanCore.ScanType.EAN8_CODE:
        return 'EAN_8';
      case scanCore.ScanType.EAN13_CODE:
        return 'EAN_13';
      case scanCore.ScanType.ITF14_CODE:
        return 'ITF';
      case scanCore.ScanType.UPC_A_CODE:
        return 'UPC_A';
      case scanCore.ScanType.UPC_E_CODE:
        return 'UPC_E';
      default:
        return 'UNKNOWN';
    }
  }
}
