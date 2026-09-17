// LxNavigator - Navigation component for LingXia apps
// Similar to WeChat mini program navigator component

export type NavigatorOpenType =
  | 'navigate'      // Push new page (default)
  | 'redirect'      // Replace current page
  | 'navigateBack'  // Go back
  | 'reLaunch'      // Restart app with new page
  | 'switchTab'     // Switch to tab page
  | 'exit'          // Exit current lxapp
  | 'tel';          // Make a phone call

// What to open. Inferred when omitted: app-id → lxapp, url → url, else page.
export type NavigatorTarget = 'page' | 'lxapp' | 'url';

// Where to open it; mirrors `as` of lx.surface.openUrl / openPage.
// A url defaults to `external`; a page without `as` follows open-type.
export type NavigatorUrlPlacement = 'external' | 'tab' | 'aside';
export type NavigatorPagePlacement = 'float' | 'window';
export type NavigatorPlacement = NavigatorUrlPlacement | NavigatorPagePlacement;
export type NavigatorEdge = 'left' | 'right' | 'top' | 'bottom';
export type NavigatorFloatPosition = 'center' | 'top' | 'bottom' | 'left' | 'right';
export type NavigatorWindowChrome = 'system' | 'full';
export type NavigatorSizeValue = number | `${number}%`;
export type NavigatorSize = { width?: NavigatorSizeValue; height?: NavigatorSizeValue };
export type NavigatorInteraction = {
  closeButton?: boolean;
  dismiss?: 'tapOutside' | 'manual';
  modal?: boolean;
};

export type NavigatorQueryValue = string | number | boolean | null | undefined;
export type NavigatorQuery = Record<string, NavigatorQueryValue>;
export type NavigatorChannel = 'release' | 'draft';

export interface LxNavigatorEventDetail {
  success?: boolean;
  errMsg?: string;
}

export interface LxNavigatorEvent extends CustomEvent<LxNavigatorEventDetail> {
  detail: LxNavigatorEventDetail;
}

type LingXiaBridgeCall = {
  invoke(route: string, params?: unknown): Promise<unknown>;
  raw: { call(method: string, params?: unknown): Promise<unknown> };
};

type NavigateOptions = {
  url: string;
  page: string | null;
  path: string | null;
  query: string | null;
  openType: NavigatorOpenType;
  target: NavigatorTarget;
  delta: number;
  placement: NavigatorPlacement | null;
  edge: string | null;
  position: string | null;
  chrome: string | null;
  size: string | null;
  interaction: string | null;
  appId: string | null;
  channel: NavigatorChannel | null;
  targetVersion: string | null;
  phoneNumber: string | null;
};

export type LxNavigatorAttributes = {
  // Navigation
  url?: string;                    // http(s) URL
  page?: string;                   // Configured page name from lxapp.json; routes are unsupported
  query?: string;                  // JSON-encoded page query params
  'open-type'?: NavigatorOpenType; // Navigation type
  target?: NavigatorTarget;        // What to open (inferred if not specified)
  delta?: number;                  // Pages to go back (for navigateBack)

  // Placement
  as?: NavigatorPlacement;
  edge?: NavigatorEdge;                // as="aside"
  position?: NavigatorFloatPosition;   // as="float"
  chrome?: NavigatorWindowChrome;      // as="window"
  size?: string;                       // JSON-encoded NavigatorSize
  interaction?: string;                // JSON-encoded NavigatorInteraction (float/window)

  // Open external lxapp
  'app-id'?: string;              // Target lxapp ID
  channel?: NavigatorChannel; // Target lxapp channel
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
      "query",
      "open-type",
      "target",
      "delta",
      "as",
      "edge",
      "position",
      "chrome",
      "size",
      "interaction",
      "app-id",
      "channel",
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
    const attr = (name: string) => this.getAttribute(name);
    const url = attr('url') || '';
    const appId = attr('app-id');
    const options: NavigateOptions = {
      url,
      page: attr('page'),
      // Read the removed attribute only to provide a forward-only runtime error
      // for untyped HTML callers instead of silently ignoring it.
      path: attr('path'),
      query: attr('query'),
      openType: (attr('open-type') || 'navigate') as NavigatorOpenType,
      target: (attr('target') as NavigatorTarget | null) ?? (appId ? 'lxapp' : url ? 'url' : 'page'),
      delta: parseInt(attr('delta') || '1', 10),
      placement: attr('as') as NavigatorPlacement | null,
      edge: attr('edge'),
      position: attr('position'),
      chrome: attr('chrome'),
      size: attr('size'),
      interaction: attr('interaction'),
      appId,
      channel: attr('channel') as NavigatorChannel | null,
      targetVersion: attr('target-version'),
      phoneNumber: attr('phone-number'),
    };
    void this.navigate(options);
  }

  private async navigate(options: NavigateOptions) {
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

  private bridge(): LingXiaBridgeCall {
    const bridge = (window as unknown as { LingXiaBridge?: LingXiaBridgeCall }).LingXiaBridge;
    if (!bridge || typeof bridge.invoke !== 'function') {
      throw new Error('LingXiaBridge is not available');
    }
    return bridge;
  }

  private async callHost(route: string, params?: unknown): Promise<void> {
    await this.bridge().invoke(route, params);
  }

  // Placements run through Logic's lx.surface, which owns surface handles.
  private async callSurface(method: 'openPage' | 'openUrl', params: unknown): Promise<void> {
    await this.bridge().raw.call(`surface.${method}`, params);
  }

  private async performNavigation(options: NavigateOptions) {
    const { url, placement } = options;
    const delta = Number.isFinite(options.delta) && options.delta > 0 ? options.delta : 1;

    if (options.path !== null) {
      throw new Error('path is not supported; pass the configured page name in page');
    }

    if (options.openType === 'tel') {
      if (!options.phoneNumber) {
        throw new Error('tel requires phone-number attribute');
      }
      await this.callHost('device.makePhoneCall', { phoneNumber: options.phoneNumber });
      return;
    }

    if (options.openType === 'exit') {
      await this.callHost('navigator.navigateBackApp');
      return;
    }

    if (options.openType === 'navigateBack') {
      if (options.target === 'lxapp') {
        await this.callHost('navigator.navigateBackApp');
      } else {
        await this.callHost('navigation.navigateBack', { delta });
      }
      return;
    }

    switch (options.target) {
      case 'url': {
        if (!url) {
          throw new Error('target url requires url');
        }
        if (!placement || placement === 'external') {
          await this.callHost('device.openUrl', { url, target: 'external' });
          return;
        }
        if (placement !== 'tab' && placement !== 'aside') {
          throw new Error(`a url opens as external, tab, or aside; got ${placement}`);
        }
        const surfaceOptions: Record<string, unknown> = { as: placement };
        if (options.edge) surfaceOptions.edge = options.edge;
        const size = this.readJsonObject('size', options.size);
        if (size) surfaceOptions.size = size;
        await this.callSurface('openUrl', { url, options: surfaceOptions });
        return;
      }
      case 'lxapp':
        if (!options.appId) {
          throw new Error('target lxapp requires app-id');
        }
        if (placement) {
          throw new Error('as is not supported for target lxapp');
        }
        await this.callHost('navigator.navigateToApp', this.buildLxAppTarget(options));
        return;
      case 'page':
        break;
      default:
        throw new Error(`Unsupported target: ${options.target}`);
    }

    const target = this.buildPageTarget(options);
    if (!target) {
      throw new Error(`${options.openType} requires page`);
    }

    if (placement) {
      if (placement !== 'float' && placement !== 'window') {
        throw new Error(`a page opens as float or window; got ${placement}`);
      }
      const surfaceOptions: Record<string, unknown> = { as: placement };
      if (target.query) surfaceOptions.query = target.query;
      if (options.position) surfaceOptions.position = options.position;
      if (options.chrome) surfaceOptions.chrome = options.chrome;
      const size = this.readJsonObject('size', options.size);
      if (size) surfaceOptions.size = size;
      const interaction = this.readJsonObject('interaction', options.interaction);
      if (interaction) surfaceOptions.interaction = interaction;
      await this.callSurface('openPage', { page: target.page, options: surfaceOptions });
      return;
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
      default:
        throw new Error(`Unsupported openType: ${options.openType}`);
    }
  }

  private readJsonObject(name: string, raw?: string | null): Record<string, unknown> | undefined {
    if (!raw) return undefined;
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      throw new Error(`${name} must be an object`);
    }
    return parsed as Record<string, unknown>;
  }

  private readQuery(raw?: string | null): NavigatorQuery | undefined {
    return this.readJsonObject('query', raw) as NavigatorQuery | undefined;
  }

  private buildPageTarget(options: {
    page?: string | null;
    query?: string | null;
  }): { page: string; query?: NavigatorQuery } | null {
    const page = options.page?.trim();
    if (!page) return null;
    const query = this.readQuery(options.query);
    return {
      page,
      ...(query ? { query } : {}),
    };
  }

  private buildLxAppTarget(options: {
    appId?: string | null;
    page?: string | null;
    query?: string | null;
    channel?: NavigatorChannel | null;
    targetVersion?: string | null;
  }): Record<string, unknown> {
    const target: Record<string, unknown> = { appId: options.appId };
    const pageTarget = this.buildPageTarget(options);
    if (pageTarget) Object.assign(target, pageTarget);
    if (options.channel) target.channel = options.channel;
    if (options.targetVersion) target.targetVersion = options.targetVersion;
    return target;
  }

}

// Register custom element
if (typeof customElements !== 'undefined' && !customElements.get('lx-navigator')) {
  customElements.define('lx-navigator', LxNavigatorElement);
}
