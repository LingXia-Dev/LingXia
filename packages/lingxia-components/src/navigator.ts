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

export interface LxNavigatorEventDetail {
  success?: boolean;
  errMsg?: string;
}

export interface LxNavigatorEvent extends CustomEvent<LxNavigatorEventDetail> {
  detail: LxNavigatorEventDetail;
}

export type LxNavigatorAttributes = {
  // Navigation
  url?: string;                    // Target URL or path
  'open-type'?: NavigatorOpenType; // Navigation type
  target?: NavigatorTarget;        // Navigation target (auto-inferred if not specified)
  delta?: number;                  // Pages to go back (for navigateBack)

  // Open external lxapp
  'app-id'?: string;              // Target lxapp ID
  path?: string;                  // Path in target lxapp (supports query string)

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
      "open-type",
      "target",
      "delta",
      "app-id",
      "path",
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
    const openType = (this.getAttribute('open-type') || 'navigate') as NavigatorOpenType;
    const explicitTarget = this.getAttribute('target') as NavigatorTarget | null;
    const delta = parseInt(this.getAttribute('delta') || '1', 10);
    const appId = this.getAttribute('app-id');
    const path = this.getAttribute('path');
    const phoneNumber = this.getAttribute('phone-number');

    // Auto-infer target if not explicitly specified
    const target = this.inferTarget(explicitTarget, url, appId);

    void this.navigate({
      url,
      openType,
      target,
      delta,
      appId,
      path,
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
    openType: NavigatorOpenType;
    target: NavigatorTarget;
    delta: number;
    appId?: string | null;
    path?: string | null;
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

  private getHostCaller(): ((name: string, payload?: any) => Promise<any>) | null {
    const host = (window as any).host;
    if (host && typeof host === 'object') {
      return (name: string, payload?: any) => {
        if (typeof host[name] !== 'function') {
          return Promise.reject(new Error(`host.${name} is not available`));
        }
        return host[name](payload);
      };
    }

    const bridge = (window as any).LingXiaBridge;
    if (bridge && typeof bridge.call === 'function') {
      return (name: string, payload?: any) => bridge.call(`host.${name}`, payload ?? null);
    }

    return null;
  }

  private async performNavigation(options: {
    url: string;
    openType: NavigatorOpenType;
    target: NavigatorTarget;
    delta: number;
    appId?: string | null;
    path?: string | null;
    phoneNumber?: string | null;
  }) {
    const caller = this.getHostCaller();
    if (!caller) {
      throw new Error('LingXia bridge not available');
    }

    const url = options.url || '';
    const delta = Number.isFinite(options.delta) && options.delta > 0 ? options.delta : 1;

    if (options.openType === 'tel') {
      if (!options.phoneNumber) {
        throw new Error('tel requires phone-number attribute');
      }
      await caller('makePhoneCall', { phoneNumber: options.phoneNumber });
      return;
    }

    if (options.openType === 'exit') {
      await caller('navigateBackLxApp');
      return;
    }

    if (options.openType === 'navigateBack') {
      if (options.target === 'lxapp') {
        await caller('navigateBackLxApp');
        return;
      }
      if (options.target === 'browser') {
        throw new Error('navigateBack is not supported for browser target');
      }
      await caller('navigateBack', { delta });
      return;
    }

    if (options.target === 'browser') {
      if (!url) {
        throw new Error('openURL requires url');
      }
      await caller('openURL', { url, target: 'external' });
      return;
    }

    if (options.target === 'lxapp') {
      if (!options.appId) {
        throw new Error('navigateToLxApp requires app-id');
      }
      const lxappPath = options.path || '';
      await caller('navigateToLxApp', { appId: options.appId, path: lxappPath });
      return;
    }

    if (options.target === 'self' && /^https?:\/\//i.test(url)) {
      if (!url) {
        throw new Error('openURL requires url');
      }
      await caller('openURL', { url, target: 'self' });
      return;
    }

    if (!url) {
      throw new Error(`${options.openType} requires url`);
    }

    switch (options.openType) {
      case 'navigate':
        await caller('navigateTo', { url });
        break;
      case 'redirect':
        await caller('redirectTo', { url });
        break;
      case 'switchTab':
        await caller('switchTab', { url });
        break;
      case 'reLaunch':
        await caller('reLaunch', { url });
        break;
      case 'openUrl':
        await caller('openURL', {
          url,
          target: options.target === 'self' ? 'self' : 'external'
        });
        break;
      default:
        throw new Error(`Unsupported openType: ${options.openType}`);
    }
  }

}

// Register custom element
if (typeof customElements !== 'undefined' && !customElements.get('lx-navigator')) {
  customElements.define('lx-navigator', LxNavigatorElement);
}
