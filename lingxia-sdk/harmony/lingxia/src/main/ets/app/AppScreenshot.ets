import { hilog } from '@kit.PerformanceAnalysisKit';
import { window } from '@kit.ArkUI';
import { image } from '@kit.ImageKit';
import { util } from '@kit.ArkTS';
import { BusinessError } from '@kit.BasicServicesKit';
import { onCallback } from 'liblingxia.so';
import { LxApp } from './LxApp';

const DOMAIN = 0x0000;
const TAG = 'LingXia.AppScreenshot';

/// Envelope shape sent back to Rust via `onCallback(_, true, JSON)`. ArkTS
/// strict mode (arkts-no-untyped-obj-literals) forbids ad-hoc object
/// literals at call sites; declaring the interface gives every literal
/// an explicit type.
interface ScreenshotEnvelope {
  ok: boolean;
  data?: string;
  error?: string;
}

/**
 * Capture a PNG snapshot of the host app's current top window. Always
 * delivers an envelope JSON via `onCallback(callbackId, true, "<json>")` —
 * `{ok:true,data:"<b64>"}` on success, `{ok:false,error:"<msg>"}` on
 * failure. The `success=false` branch of
 * `lingxia/src/ffi/harmony.rs::on_callback` tries to parse data as a u32
 * error code and discards anything else, which would collapse our
 * diagnostic messages into BusinessError(1000). Always-true + envelope
 * side-steps that limitation.
 *
 * Counterpart of Android's `com.lingxia.app.AppScreenshot.captureWindow`.
 */
export function captureAppScreenshot(callbackIdStr: string): boolean {
  const deliverEnvelope = (envelope: ScreenshotEnvelope): void => {
    try {
      onCallback(callbackIdStr, true, JSON.stringify(envelope));
    } catch (callbackError) {
      hilog.error(DOMAIN, TAG, `Failed to deliver app screenshot envelope: ${callbackError}`);
    }
  };
  const fail = (reason: string): boolean => {
    const envelope: ScreenshotEnvelope = { ok: false, error: reason };
    deliverEnvelope(envelope);
    return false;
  };

  try {
    const ctx = LxApp.context;
    if (!ctx) {
      return fail('LxApp UIAbility context is not available');
    }
    // BusinessError convention in OHOS varies: some APIs pass `null` on
    // success, others pass `{code:0, message:''}`. Always treat code 0 as
    // success to handle both shapes.
    window.getLastWindow(ctx, (err: BusinessError, win: window.Window) => {
      if (err && err.code !== 0) {
        fail(`getLastWindow failed: code=${err.code} msg=${err.message}`);
        return;
      }
      try {
        win.snapshot((snapshotErr: BusinessError, pixelMap: image.PixelMap) => {
          if (snapshotErr && snapshotErr.code !== 0) {
            fail(`window.snapshot failed: code=${snapshotErr.code} msg=${snapshotErr.message}`);
            return;
          }
          if (!pixelMap) {
            fail('window.snapshot returned no PixelMap');
            return;
          }
          try {
            const packer = image.createImagePacker();
            const opts: image.PackingOption = { format: 'image/png', quality: 100 };
            packer.packing(
              pixelMap,
              opts,
              (packError: BusinessError | null, buffer: ArrayBuffer) => {
                try {
                  if (packError && packError.code !== 0) {
                    fail(`PNG packing failed: code=${packError.code} msg=${packError.message}`);
                    return;
                  }
                  const b64 = new util.Base64Helper().encodeToStringSync(new Uint8Array(buffer));
                  const envelope: ScreenshotEnvelope = { ok: true, data: b64 };
                  deliverEnvelope(envelope);
                } finally {
                  try { packer.release(); } catch (_) { /* ignore */ }
                  try { pixelMap.release(); } catch (_) { /* ignore */ }
                }
              }
            );
          } catch (packerError) {
            fail(`Failed to create image packer: ${packerError}`);
          }
        });
      } catch (snapshotThrow) {
        fail(`window.snapshot threw: ${snapshotThrow}`);
      }
    });
    return true;
  } catch (error) {
    return fail(`captureAppScreenshot threw: ${error}`);
  }
}
