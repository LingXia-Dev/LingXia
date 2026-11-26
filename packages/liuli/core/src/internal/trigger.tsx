import { childrenToArray, cloneElement, createContext, cx, elementType, useContext, type Child } from '@lingxia/view';
import { triggerRecipe as r } from './trigger.recipe';
import { dataOf } from './data';

/** What an overlay asks of its trigger: the panel it toggles, the hint it shows, the note that describes it. */
export interface TriggerWiring {
  popoverTarget?: string;
  interestFor?: string;
  describedBy?: string;
  /** The overlay's test id and `data-*`, for a pressable trigger that has no test id of its own. */
  identity?: Record<string, unknown>;
}

/** The prop an overlay standing as another's trigger (a Tooltip) receives that overlay's wiring in. */
export const PASSED_WIRING = 'lxPassedWiring';

const TriggerContext = createContext<TriggerWiring | null>(null);

/** Liuli's own pressables, which take the wiring from context instead of being wrapped. */
const pressables = new Set<unknown>();
export function registerPressable(component: unknown) {
  pressables.add(component);
}

/** Overlays that may stand as another overlay's trigger, so one button opens a panel and shows a hint. */
const wrappers = new Set<unknown>();
export function registerTriggerWrapper(component: unknown) {
  wrappers.add(component);
}

/** Liuli's other controls (a Switch), which a row keeps at its end but an overlay does not wire. */
const controls = new Set<unknown>();
export function registerControl(component: unknown) {
  controls.add(component);
}

/** Whether any top-level node of `children` is a control: a Liuli pressable or a plain button. */
export function holdsControl(children: Child): boolean {
  return childrenToArray(children).some(child => typeof child === 'object' && child !== null && (pressables.has(elementType(child)) || controls.has(elementType(child)) || elementType(child) === 'button'));
}

/**
 * A pressable's test id and `data-*`: its own, else, as an overlay's trigger, the overlay's, so
 * `<Tooltip data-testid="help">` around a Button still finds the Button.
 */
export function identityOf(testId: string | undefined, attributes: object, trigger: TriggerWiring | null): Record<string, unknown> {
  if (testId !== undefined || trigger?.identity === undefined) return { 'data-testid': testId, ...dataOf(attributes) };
  return { ...trigger.identity, ...dataOf(attributes) };
}

/** For a pressable: the wiring of the overlay it opens, if it is one's trigger. */
export function useTrigger() {
  return useContext(TriggerContext);
}

/**
 * An overlay's `children`, wired as its trigger. A Liuli pressable reads the wiring from context;
 * any other element is wrapped in one unstyled button, one focus stop named by its content.
 */
export function Trigger({ children, wiring: own, passed, className, 'data-testid': testId, ...attributes }: { children: Child; wiring: TriggerWiring; passed?: TriggerWiring; className?: string; 'data-testid'?: string }) {
  const nodes = childrenToArray(children);
  if (nodes.length !== 1) throw Error('UI_PROP: an overlay takes one element as its trigger');
  const [child] = nodes;
  // An outer overlay this one is the trigger of hands its wiring down; the one button takes both.
  const wiring = passed ? { ...passed, ...own } : own;
  if (typeof child === 'object' && wrappers.has(elementType(child))) {
    return cloneElement(child as never, { [PASSED_WIRING]: wiring });
  }
  if (typeof child === 'object' && pressables.has(elementType(child))) {
    return <TriggerContext.Provider value={{ ...wiring, identity: { 'data-testid': testId, ...dataOf(attributes) } }}>{child}</TriggerContext.Provider>;
  }
  return (
    <button
      popoverTarget={wiring.popoverTarget}
      interestFor={wiring.interestFor}
      aria-describedby={wiring.describedBy}
      data-testid={testId} {...dataOf(attributes)}
      className={cx(className ?? r.wrap.base, wiring.interestFor !== undefined && r.wrap.hint)}
    >
      {child}
    </button>
  );
}
