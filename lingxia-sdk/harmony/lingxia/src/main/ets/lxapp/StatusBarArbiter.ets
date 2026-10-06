/**
 * The in-app browser covers every container and owns the status bar while it
 * is open. Containers keep tracking what they want and re-derive it when the
 * browser lets go, so changes made underneath it are not lost.
 */
export class StatusBarArbiter {
  private static suspended: boolean = false;
  private static releaseHandler: (() => void) | null = null;

  public static isSuspended(): boolean {
    return StatusBarArbiter.suspended;
  }

  public static setReleaseHandler(handler: () => void): void {
    StatusBarArbiter.releaseHandler = handler;
  }

  public static suspend(suspended: boolean): void {
    if (StatusBarArbiter.suspended === suspended) {
      return;
    }
    StatusBarArbiter.suspended = suspended;
    const handler = StatusBarArbiter.releaseHandler;
    if (!suspended && handler) {
      handler();
    }
  }
}
