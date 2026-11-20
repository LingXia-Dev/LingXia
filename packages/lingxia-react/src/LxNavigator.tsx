import React from 'react';
import type {
  LinkTarget,
  LxNavigatorAttributes,
  LxNavigatorEvent,
  NavigatorEdge,
  NavigatorFloatPosition,
  NavigatorInteraction,
  NavigatorPlacement,
  NavigatorSize,
  NavigatorWindowChrome,
} from '@lingxia/elements';
import { buildNavigatorNativeAttrs } from '@lingxia/elements';

// Import to ensure custom element is registered
import '@lingxia/elements';

export interface LxNavigatorProps extends Omit<LxNavigatorAttributes, 'onSuccess' | 'onFail' | 'onComplete' | 'to' | 'size' | 'interaction'> {
  /** Where a tap goes: `{ page, params }`, `{ tab }`, `{ app }`, `{ url }` or `{ back: n }`. */
  to: LinkTarget;
  /** `{ page }`: replace the current page instead of pushing. */
  replace?: boolean;
  /** `{ page }` or `{ tab }`: clear the stack to it. */
  reset?: boolean;

  // Placement
  as?: NavigatorPlacement;
  edge?: NavigatorEdge;
  position?: NavigatorFloatPosition;
  chrome?: NavigatorWindowChrome;
  size?: NavigatorSize;
  interaction?: NavigatorInteraction;

  // Hover effect
  hoverClass?: string;
  hoverStopPropagation?: boolean;
  hoverStartTime?: number;
  hoverStayTime?: number;

  // Styling
  className?: string;
  style?: React.CSSProperties;

  // Events
  onSuccess?: (e: LxNavigatorEvent) => void;
  onFail?: (e: LxNavigatorEvent) => void;
  onComplete?: (e: LxNavigatorEvent) => void;

  // Children
  children?: React.ReactNode;
}

export const LxNavigator = React.forwardRef<HTMLElement, LxNavigatorProps>(
  (props, ref) => {
    const {
      to,
      replace,
      reset,
      as,
      edge,
      position,
      chrome,
      size,
      interaction,
      hoverClass = 'navigator-hover',
      hoverStopPropagation = false,
      hoverStartTime = 20,
      hoverStayTime = 70,
      className,
      style,
      onSuccess,
      onFail,
      onComplete,
      children,
      ...rest
    } = props;

    const elementRef = React.useRef<HTMLElement>(null);

    React.useImperativeHandle(ref, () => elementRef.current!);

    React.useEffect(() => {
      const element = elementRef.current;
      if (!element) return;

      const handleSuccess = (e: Event) => {
        if (onSuccess) {
          onSuccess(e as LxNavigatorEvent);
        }
      };

      const handleFail = (e: Event) => {
        if (onFail) {
          onFail(e as LxNavigatorEvent);
        }
      };

      const handleComplete = (e: Event) => {
        if (onComplete) {
          onComplete(e as LxNavigatorEvent);
        }
      };
      const successListener: EventListenerObject = { handleEvent: handleSuccess };
      const failListener: EventListenerObject = { handleEvent: handleFail };
      const completeListener: EventListenerObject = { handleEvent: handleComplete };

      element.addEventListener('success', successListener);
      element.addEventListener('fail', failListener);
      element.addEventListener('complete', completeListener);

      return () => {
        element.removeEventListener('success', successListener);
        element.removeEventListener('fail', failListener);
        element.removeEventListener('complete', completeListener);
      };
    }, [onSuccess, onFail, onComplete]);

    const navigatorProps = buildNavigatorNativeAttrs({
      to,
      replace,
      reset,
      as,
      edge,
      position,
      chrome,
      size,
      interaction,
      hoverClass,
      hoverStopPropagation,
      hoverStartTime,
      hoverStayTime,
    }, rest as Record<string, unknown>);

    return React.createElement('lx-navigator', {
      ref: elementRef,
      className,
      style,
      ...navigatorProps
    }, children);
  }
);

LxNavigator.displayName = 'LxNavigator';
