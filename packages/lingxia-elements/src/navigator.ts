// LxNavigator - Navigation component for LingXia apps
// Similar to WeChat mini program navigator component

export type NavigatorOpenType =
  | 'navigate'      // Push new page (default)
  | 'redirect'      // Replace current page
  | 'navigateBack'  // Go back
  | 'reLaunch'      // Restart app with new page
  | 'switchTab'     // Switch to tab page
  | 'exit'          // Exit current lxapp
  | 'openUrl'       // Open external URL or lxapp
  | 'tel';          // Make a phone call

export type NavigatorTarget =
  | 'self'          // Navigate within current lxapp (default)
  | 'lxapp'         // Open another lxapp
  | 'browser';      // Open in external browser

export type NavigatorQueryValue = string | number | boolean | null | undefined;
export type NavigatorQuery = Record<string, NavigatorQueryValue>;
export type NavigatorEnvVersion = 'release' | 'preview' | 'develop';

export interface LxNavigatorEventDetail {
  success?: boolean;
  errMsg?: string;
}

export interface LxNavigatorEvent extends CustomEvent<LxNavigatorEventDetail> {
  detail: LxNavigatorEventDetail;
}

type LingXiaBridgeCall = {
  call(method: string, params?: unknown, options?: { cap?: string }): Promise<unknown>;
};

export type LxNavigatorAttributes = {
  // Navigation
  url?: string;                    // Browser URL for openUrl/browser target
  page?: string;                   // Named page in lxapp.json
  path?: string;                   // Raw page path, supports query string
  query?: string;                  // JSON-encoded page query params
  'open-type'?: NavigatorOpenType; // Navigation type
  target?: NavigatorTarget;        // Navigation target (auto-inferred if not specified)
  delta?: number;                  // Pages to go back (for navigateBack)

  // Open external lxapp
  'app-id'?: string;              // Target lxapp ID
  'env-version'?: NavigatorEnvVersion; // Target lxapp envVersion
  'target-version'?: string;      // Exact target lxapp version

  // Phone call
  'phone-number'?: string;        // Phone number for tel open-type

  // Hover effect
  'hover-class'?: string;         // CSS class for hover state
  'hover-stop-propagation'?: boolean; // Prevent hover propagation
  'hover-start-time'?: number;    // Hover start delay (ms)
  'hover-stay-time'?: number;     // Hover stay duration (ms)

  // Styling
  className?: string;
  style?: any;

  // Events
  onSuccess?: (e: LxNavigatorEvent) => void;
  onFail?: (e: LxNavigatorEvent) => void;
  onComplete?: (e: LxNavigatorEvent) => void;

  // React
  ref?: any;
  children?: any;
};

declare global {
  namespace JSX {
    interface IntrinsicElements {
      "lx-navigator": LxNavigatorAttributes;
    }
  }
}

// Component implementation
export class LxNavigatorElement extends HTMLElement {
  static get observedAttributes() {
    return [
      "url",
      "page",
      "path",
      "query",
      "open-type",
      "target",
      "delta",
      "app-id",
      "env-version",
      "target-version",
      "phone-number",
      "hover-class",
      "hover-stop-propagation",
      "hover-start-time",
      "hover-stay-time"
    ];
  }

  private hoverClass: string = 'navigator-hover';
  private hoverStopPropagation: boolean = false;
  private hoverStartTime: number = 20;
  private hoverStayTime: number = 70;
  private hoverTimer: number | null = null;
  private isHovering: boolean = false;
  private readonly onClick = (e: MouseEvent) => this.handleClick(e);
  private readonly onTouchStart = (e: TouchEvent) => this.handleTouchStart(e);
  private readonly onTouchEnd = (e: TouchEvent) => this.handleTouchEnd(e);
  private readonly onTouchCancel = (e: TouchEvent) => this.handleTouchCancel(e);
  private readonly onMouseEnter = (e: MouseEvent) => this.handleMouseEnter(e);
  private readonly onMouseLeave = (e: MouseEvent) => this.handleMouseLeave(e);
  private readonly clickListener: EventListenerObject = {
    handleEvent: (event: Event): void => this.onClick(event as MouseEvent),
  };
  private readonly touchStartListener: EventListenerObject = {
    handleEvent: (event: Event): void => this.onTouchStart(event as TouchEvent),
  };
  private readonly touchEndListener: EventListenerObject = {
    handleEvent: (event: Event): void => this.onTouchEnd(event as TouchEvent),
  };
  private readonly touchCancelListener: EventListenerObject = {
    handleEvent: (event: Event): void => this.onTouchCancel(event as TouchEvent),
  };
  private readonly mouseEnterListener: EventListenerObject = {
    handleEvent: (event: Event): void => this.onMouseEnter(event as MouseEvent),
  };
  private readonly mouseLeaveListener: EventListenerObject = {
    handleEvent: (event: Event): void => this.onMouseLeave(event as MouseEvent),
  };

  connectedCallback() {
    this.setupHoverEffect();
    this.setupClickHandler();
    this.applyDefaultStyles();
  }

  disconnectedCallback() {
    this.clearHoverTimer();
    this.removeEventListener('click', this.clickListener);
    this.removeEventListener('touchstart', this.touchStartListener);
    this.removeEventListener('touchend', this.touchEndListener);
    this.removeEventListener('touchcancel', this.touchCancelListener);
    this.removeEventListener('mouseenter', this.mouseEnterListener);
    this.removeEventListener('mouseleave', this.mouseLeaveListener);
  }

  attributeChangedCallback(name: string, oldValue: string | null, newValue: string | null) {
    if (oldValue === newValue) return;

    switch (name) {
      case 'hover-class':
        this.hoverClass = newValue || 'navigator-hover';
        break;
      case 'hover-stop-propagation':
        this.hoverStopPropagation = newValue === 'true';
        break;
      case 'hover-start-time':
        this.hoverStartTime = parseInt(newValue || '20', 10);
        break;
      case 'hover-stay-time':
        this.hoverStayTime = parseInt(newValue || '70', 10);
        break;
    }
  }

  private applyDefaultStyles() {
    if (!this.hasAttribute('style') && !this.className) {
      this.style.display = 'inline-block';
      this.style.cursor = 'pointer';
      this.style.userSelect = 'none';
      (this.style as any).webkitTapHighlightColor = 'transparent';
    }
  }

  private setupHoverEffect() {
    this.addEventListener('touchstart', this.touchStartListener, { passive: true });
    this.addEventListener('touchend', this.touchEndListener, { passive: true });
    this.addEventListener('touchcancel', this.touchCancelListener, { passive: true });

    // Desktop hover
    this.addEventListener('mouseenter', this.mouseEnterListener);
    this.addEventListener('mouseleave', this.mouseLeaveListener);
  }

  private setupClickHandler() {
    this.addEventListener('click', this.clickListener);
  }

  private handleTouchStart(e: TouchEvent) {
    if (this.hoverStopPropagation) {
      e.stopPropagation();
    }

    this.clearHoverTimer();
    this.hoverTimer = window.setTimeout(() => {
      this.addHoverClass();
    }, this.hoverStartTime);
  }

  private handleTouchEnd(_e: TouchEvent) {
    this.clearHoverTimer();
    if (this.isHovering) {
      window.setTimeout(() => {
        this.removeHoverClass();
      }, this.hoverStayTime);
    }
  }

  private handleTouchCancel(_e: TouchEvent) {
    this.clearHoverTimer();
    this.removeHoverClass();
  }

  private handleMouseEnter(_e: MouseEvent) {
    this.clearHoverTimer();
    this.hoverTimer = window.setTimeout(() => {
      this.addHoverClass();
    }, this.hoverStartTime);
  }

  private handleMouseLeave(_e: MouseEvent) {
    this.clearHoverTimer();
    if (this.isHovering) {
      window.setTimeout(() => {
        this.removeHoverClass();
      }, this.hoverStayTime);
    }
  }

  private addHoverClass() {
    if (this.hoverClass && this.hoverClass !== 'none') {
      this.classList.add(this.hoverClass);
      this.isHovering = true;
    }
  }

  private removeHoverClass() {
    if (this.hoverClass && this.hoverClass !== 'none') {
      this.classList.remove(this.hoverClass);
      this.isHovering = false;
    }
  }

  private clearHoverTimer() {
    if (this.hoverTimer !== null) {
      window.clearTimeout(this.hoverTimer);
      this.hoverTimer = null;
    }
  }

  private handleClick(e: MouseEvent) {
    e.preventDefault();

    const url = this.getAttribute('url') || '';
    const page = this.getAttribute('page');
    const path = this.getAttribute('path');
    const query = this.getAttribute('query');
    const openType = (this.getAttribute('open-type') || 'navigate') as NavigatorOpenType;
    const explicitTarget = this.getAttribute('target') as NavigatorTarget | null;
    const delta = parseInt(this.getAttribute('delta') || '1', 10);
    const appId = this.getAttribute('app-id');
    const envVersion = this.getAttribute('env-version') as NavigatorEnvVersion | null;
    const targetVersion = this.getAttribute('target-version');
    const phoneNumber = this.getAttribute('phone-number');

    // Auto-infer target if not explicitly specified
    const target = this.inferTarget(explicitTarget, url, appId);

    void this.navigate({
      url,
      page,
      path,
      query,
      openType,
      target,
      delta,
      appId,
      envVersion,
      targetVersion,
      phoneNumber
    });
  }

  /**
   * Auto-infer navigation target based on context
   * Priority: explicit target > appId > HTTPS URL > default (self)
   */
  private inferTarget(
    explicitTarget: NavigatorTarget | null,
    url: string,
    appId: string | null
  ): NavigatorTarget {
    // 1. Explicit target has highest priority
    if (explicitTarget) {
      return explicitTarget;
    }

    // 2. If appId is specified, target is another lxapp
    if (appId) {
      return 'lxapp';
    }

    // 3. If URL starts with http:// or https://, open in browser
    if (/^https?:\/\//i.test(url)) {
      return 'browser';
    }

    // 4. Default: navigate within current lxapp
    return 'self';
  }

  private async navigate(options: {
    url: string;
    page?: string | null;
    path?: string | null;
    query?: string | null;
    openType: NavigatorOpenType;
    target: NavigatorTarget;
    delta: number;
    appId?: string | null;
    envVersion?: NavigatorEnvVersion | null;
    targetVersion?: string | null;
    phoneNumber?: string | null;
  }) {
    try {
      await this.performNavigation(options);
      this.dispatchSuccess();
    } catch (error) {
      this.dispatchFail(error);
    }
  }

  private dispatchSuccess() {
    const successEvent = new CustomEvent('success', {
      detail: {
        success: true
      },
      bubbles: true,
      composed: true
    });
    this.dispatchEvent(successEvent);

    const completeEvent = new CustomEvent('complete', {
      detail: {
        success: true
      },
      bubbles: true,
      composed: true
    });
    this.dispatchEvent(completeEvent);
  }

  private resolveErrorMessage(error: unknown): string {
    if (error instanceof Error) {
      const message = error.message.trim();
      return message || 'Unknown error';
    }

    if (typeof error === 'string') {
      const message = error.trim();
      return message || 'Unknown error';
    }

    if (error && typeof error === 'object') {
      const message = (error as { message?: unknown }).message;
      if (typeof message === 'string' && message.trim()) {
        return message;
      }
    }

    return 'Unknown error';
  }

  private dispatchFail(error: unknown) {
    const errMsg = this.resolveErrorMessage(error);
    const failEvent = new CustomEvent('fail', {
      detail: {
        success: false,
        errMsg
      },
      bubbles: true,
      composed: true
    });
    this.dispatchEvent(failEvent);

    const completeEvent = new CustomEvent('complete', {
      detail: {
        success: false,
        errMsg
      },
      bubbles: true,
      composed: true
    });
    this.dispatchEvent(completeEvent);
  }

  private callHost(route: string, params?: unknown): Promise<void> {
    const bridge = (window as unknown as { LingXiaBridge?: LingXiaBridgeCall }).LingXiaBridge;
    if (!bridge || typeof bridge.call !== 'function') {
      return Promise.reject(new Error('LingXiaBridge is not available'));
    }
    return bridge.call(`host.${route}`, params, { cap: 'host' }).then(() => undefined);
  }

  private async performNavigation(options: {
    url: string;
    page?: string | null;
    path?: string | null;
    query?: string | null;
    openType: NavigatorOpenType;
    target: NavigatorTarget;
    delta: number;
    appId?: string | null;
    envVersion?: NavigatorEnvVersion | null;
    targetVersion?: string | null;
    phoneNumber?: string | null;
  }) {
    const url = options.url || '';
    const delta = Number.isFinite(options.delta) && options.delta > 0 ? options.delta : 1;

    if (options.openType === 'tel') {
      if (!options.phoneNumber) {
        throw new Error('tel requires phone-number attribute');
      }
      await this.callHost('device.makePhoneCall', { phoneNumber: options.phoneNumber });
      return;
    }

    if (options.openType === 'exit') {
      await this.callHost('navigator.navigateBackLxApp');
      return;
    }

    if (options.openType === 'navigateBack') {
      if (options.target === 'lxapp') {
        await this.callHost('navigator.navigateBackLxApp');
        return;
      }
      if (options.target === 'browser') {
        throw new Error('navigateBack is not supported for browser target');
      }
      await this.callHost('navigation.navigateBack', { delta });
      return;
    }

    if (options.target === 'browser') {
      if (!url) {
        throw new Error('openUrl requires url');
      }
      await this.callHost('device.openUrl', { url, target: 'external' });
      return;
    }

    if (options.target === 'lxapp') {
      if (!options.appId) {
        throw new Error('navigateToLxApp requires app-id');
      }
      await this.callHost('navigator.navigateToLxApp', this.buildLxAppTarget(options));
      return;
    }

    if (options.target === 'self' && /^https?:\/\//i.test(url)) {
      if (!url) {
        throw new Error('openUrl requires url');
      }
      await this.callHost('device.openUrl', { url, target: 'self' });
      return;
    }

    const target = this.buildPageTarget(options);
    if (!target) {
      throw new Error(`${options.openType} requires page or path`);
    }

    switch (options.openType) {
      case 'navigate':
        await this.callHost('navigation.navigateTo', target);
        break;
      case 'redirect':
        await this.callHost('navigation.redirectTo', target);
        break;
      case 'switchTab':
        await this.callHost('navigation.switchTab', target);
        break;
      case 'reLaunch':
        await this.callHost('navigation.reLaunch', target);
        break;
      case 'openUrl':
        await this.callHost('device.openUrl', {
          url,
          target: options.target === 'self' ? 'self' : 'external'
        });
        break;
      default:
        throw new Error(`Unsupported openType: ${options.openType}`);
    }
  }

  private readQuery(raw?: string | null): NavigatorQuery | undefined {
    if (!raw) return undefined;
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      throw new Error('query must be an object');
    }
    return parsed as NavigatorQuery;
  }

  private buildPageTarget(options: {
    page?: string | null;
    path?: string | null;
    query?: string | null;
  }): { page?: string; path?: string; query?: NavigatorQuery } | null {
    const page = options.page?.trim();
    const path = options.path?.trim();
    if (page && path) {
      throw new Error('pass either page or path, not both');
    }
    if (!page && !path) return null;
    const query = this.readQuery(options.query);
    return {
      ...(page ? { page } : { path: path! }),
      ...(query ? { query } : {}),
    };
  }

  private buildLxAppTarget(options: {
    appId?: string | null;
    page?: string | null;
    path?: string | null;
    query?: string | null;
    envVersion?: NavigatorEnvVersion | null;
    targetVersion?: string | null;
  }): Record<string, unknown> {
    const target: Record<string, unknown> = { appId: options.appId };
    const pageTarget = this.buildPageTarget(options);
    if (pageTarget) Object.assign(target, pageTarget);
    if (options.envVersion) target.envVersion = options.envVersion;
    if (options.targetVersion) target.targetVersion = options.targetVersion;
    return target;
  }

}

// Register custom element
if (typeof customElements !== 'undefined' && !customElements.get('lx-navigator')) {
  customElements.define('lx-navigator', LxNavigatorElement);
}
