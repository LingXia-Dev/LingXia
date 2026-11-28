// LxNavigator - a tappable link to a navigation target, for WebView pages.

/** A JSON value a page param can hold. */
export type NavigatorParamValue =
  | string
  | number
  | boolean
  | null
  | NavigatorParamValue[]
  | { [key: string]: NavigatorParamValue };
export type NavigatorParams = Record<string, NavigatorParamValue>;
export type NavigatorChannel = 'release' | 'draft';

/**
 * Where a tap goes: the targets of `lx.nav.go`, plus `{ back }`. Each branch
 * types the others' keys `never`.
 */
export type LinkTarget =
  | { page: string; params?: NavigatorParams; tab?: never; app?: never; url?: never; back?: never }
  | { tab: string; page?: never; app?: never; url?: never; back?: never; params?: never }
  | {
      app: string;
      page?: string;
      params?: NavigatorParams;
      channel?: NavigatorChannel;
      version?: string;
      tab?: never;
      url?: never;
      back?: never;
    }
  | { url: string; page?: never; tab?: never; app?: never; back?: never; params?: never }
  | { back: number; page?: never; tab?: never; app?: never; url?: never; params?: never };

// Where to open it; mirrors `as` of lx.surface.open. A url
// opens in the in-app browser unless `as="external"` hands it to the system.
export type NavigatorUrlPlacement = 'external' | 'main' | 'aside';
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
  to: LinkTarget;
  replace: boolean;
  reset: boolean;
  placement: NavigatorPlacement | null;
  edge: string | null;
  position: string | null;
  chrome: string | null;
  size: string | null;
  interaction: string | null;
};

export type LxNavigatorAttributes = {
  // Navigation
  to?: string;                     // JSON-encoded LinkTarget
  replace?: boolean;               // { page }: replace the current page instead of pushing
  reset?: boolean;                 // { page } | { tab }: clear the stack to it

  // Placement
  as?: NavigatorPlacement;
  edge?: NavigatorEdge;                // as="aside"
  position?: NavigatorFloatPosition;   // as="float"
  chrome?: NavigatorWindowChrome;      // as="window"
  size?: string;                       // JSON-encoded NavigatorSize
  interaction?: string;                // JSON-encoded NavigatorInteraction (float/window)

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
      "to",
      "replace",
      "reset",
      "as",
      "edge",
      "position",
      "chrome",
      "size",
      "interaction",
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

  /** The target, when set as a property rather than the `to` attribute. */
  to: LinkTarget | undefined;

  private handleClick(e: MouseEvent) {
    e.preventDefault();
    const attr = (name: string) => this.getAttribute(name);
    const flag = (name: string) => {
      const value = attr(name);
      return value !== null && value !== 'false';
    };
    let to: LinkTarget | undefined;
    try {
      to = this.to ?? (this.readJsonObject('to', attr('to')) as LinkTarget | undefined);
      if (!to) throw new Error('lx-navigator needs a to target');
    } catch (error) {
      this.dispatchFail(error);
      return;
    }
    void this.navigate({
      to,
      replace: flag('replace'),
      reset: flag('reset'),
      placement: attr('as') as NavigatorPlacement | null,
      edge: attr('edge'),
      position: attr('position'),
      chrome: attr('chrome'),
      size: attr('size'),
      interaction: attr('interaction'),
    });
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
    const { to, placement } = options;
    // An app target may also name its page.
    const named = (['page', 'tab', 'app', 'url', 'back'] as const)
      .filter((key) => to[key] !== undefined && !(key === 'page' && to.app !== undefined));
    if (named.length !== 1) {
      throw new Error('to names exactly one of page, tab, app, url or back');
    }
    const isPage = named[0] === 'page';
    if (options.replace && !isPage) throw new Error('replace applies to { page }');
    if (options.reset && !isPage && to.tab === undefined) {
      throw new Error('reset applies to { page } or { tab }');
    }
    if (placement && !isPage && to.url === undefined) {
      throw new Error('as applies to { page } or { url }');
    }

    if (to.back !== undefined) {
      if (!Number.isInteger(to.back) || to.back < 1) throw new Error('back must be a positive integer');
      await this.callHost('navigation.navigateBack', { delta: to.back });
      return;
    }

    if (to.tab !== undefined) {
      await this.callHost(options.reset ? 'navigation.reLaunch' : 'navigation.switchTab', { page: to.tab });
      return;
    }

    if (to.app !== undefined) {
      const target: Record<string, unknown> = { appId: to.app };
      if (to.page) target.page = to.page;
      if (to.params) target.query = to.params;
      if (to.channel) target.channel = to.channel;
      if (to.version) target.targetVersion = to.version;
      await this.callHost('navigator.navigateToApp', target);
      return;
    }

    if (to.url !== undefined) {
      if (to.url.startsWith('tel:')) {
        await this.callHost('device.makePhoneCall', { phoneNumber: to.url.slice('tel:'.length) });
        return;
      }
      if (placement === 'external') {
        await this.callHost('device.openUrl', { url: to.url, target: 'external' });
        return;
      }
      if (placement && placement !== 'main' && placement !== 'aside') {
        throw new Error(`a url opens as external, main, or aside; got ${placement}`);
      }
      const surfaceOptions: Record<string, unknown> = {};
      if (placement) surfaceOptions.as = placement;
      if (options.edge) surfaceOptions.edge = options.edge;
      const size = this.readJsonObject('size', options.size);
      if (size) surfaceOptions.size = size;
      await this.callSurface('openUrl', { url: to.url, options: surfaceOptions });
      return;
    }

    const page = to.page?.trim();
    if (!page) throw new Error('page must be a configured page name');

    if (placement) {
      if (placement !== 'float' && placement !== 'window') {
        throw new Error(`a page opens as float or window; got ${placement}`);
      }
      if (options.replace || options.reset) throw new Error('as cannot combine with replace or reset');
      const surfaceOptions: Record<string, unknown> = { as: placement };
      if (to.params) surfaceOptions.params = to.params;
      if (options.position) surfaceOptions.position = options.position;
      if (options.chrome) surfaceOptions.chrome = options.chrome;
      const size = this.readJsonObject('size', options.size);
      if (size) surfaceOptions.size = size;
      const interaction = this.readJsonObject('interaction', options.interaction);
      if (interaction) surfaceOptions.interaction = interaction;
      await this.callSurface('openPage', { page, options: surfaceOptions });
      return;
    }

    const target = { page, ...(to.params ? { query: to.params } : {}) };
    if (options.reset) await this.callHost('navigation.reLaunch', target);
    else if (options.replace) await this.callHost('navigation.redirectTo', target);
    else await this.callHost('navigation.navigateTo', target);
  }

  private readJsonObject(name: string, raw?: string | null): Record<string, unknown> | undefined {
    if (!raw) return undefined;
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      throw new Error(`${name} must be an object`);
    }
    return parsed as Record<string, unknown>;
  }

}

// Register custom element
if (typeof customElements !== 'undefined' && !customElements.get('lx-navigator')) {
  customElements.define('lx-navigator', LxNavigatorElement);
}
