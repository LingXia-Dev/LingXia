import { forwardHostLog } from 'liblingxia.so';

/**
 * Routes SDK-native logs into the LingXia Rust log pipeline.
 *
 * Unlike direct hilog calls, records emitted here flow through the same pipeline
 * as Rust logs, so they show up in dev-server log streams tagged with the
 * originating appId/path when provided.
 */
export class LxLog {
  private static readonly VERBOSE: number = 0;
  private static readonly DEBUG: number = 1;
  private static readonly INFO: number = 2;
  private static readonly WARN: number = 3;
  private static readonly ERROR: number = 4;

  public static v(tag: string, message: string, appId: string = '', path: string = ''): boolean {
    return forwardHostLog(LxLog.VERBOSE, tag, appId, path, message);
  }

  public static d(tag: string, message: string, appId: string = '', path: string = ''): boolean {
    return forwardHostLog(LxLog.DEBUG, tag, appId, path, message);
  }

  public static i(tag: string, message: string, appId: string = '', path: string = ''): boolean {
    return forwardHostLog(LxLog.INFO, tag, appId, path, message);
  }

  public static w(tag: string, message: string, appId: string = '', path: string = ''): boolean {
    return forwardHostLog(LxLog.WARN, tag, appId, path, message);
  }

  public static e(tag: string, message: string, appId: string = '', path: string = ''): boolean {
    return forwardHostLog(LxLog.ERROR, tag, appId, path, message);
  }
}
