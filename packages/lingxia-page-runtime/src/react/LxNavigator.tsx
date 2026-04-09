import React from 'react';
import type {
  LxNavigatorAttributes,
  LxNavigatorEvent,
  NavigatorOpenType,
  NavigatorTarget
} from '@lingxia/elements';
import { buildNavigatorNativeAttrs } from '@lingxia/elements';

// Import to ensure custom element is registered
import '@lingxia/elements';

export interface LxNavigatorProps extends Omit<LxNavigatorAttributes, 'onSuccess' | 'onFail' | 'onComplete'> {
  // Navigation
  url?: string;
  openType?: NavigatorOpenType;
  target?: NavigatorTarget; // Auto-inferred if not specified
  delta?: number;

  // Open external lxapp
  appId?: string;
  path?: string; // Supports query string

  // Phone call
  phoneNumber?: string;

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
      url,
      openType = 'navigate',
      target, // Auto-inferred, no default
      delta = 1,
      appId,
      path,
      phoneNumber,
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
      url,
      openType,
      target,
      delta,
      appId,
      path,
      phoneNumber,
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
