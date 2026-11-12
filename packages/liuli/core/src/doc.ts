// The shape of each component's `X.doc.ts`: what the code cannot say. Props come from the exported
// props type and its JSDoc (`@default` included), classes from the recipe, so neither is repeated here.

import type { PatternName } from './a11y/patterns';

export type Category =
  | 'action' | 'container' | 'content' | 'feedback' | 'form' | 'layout' | 'list' | 'navigation' | 'overlay';

export interface ComponentDoc {
  /** Component name, equal to its export and its folder. */
  readonly name: string;
  readonly category: Category;
  /** `lab` may still change; `stable` once its accessibility contract passes on all four platforms. */
  readonly status: 'stable' | 'lab';
  /** One line for the catalog and the skill index; no trailing period. */
  readonly summary: string;
  /** What it is for, in two or three sentences. */
  readonly description: string;
  /** Search terms an agent or a person might use instead of the name. */
  readonly keywords: readonly string[];
  /** The exported props type the generator reads (`ButtonProps`). */
  readonly propsType: string;
  /** The exported recipe and its part manifest (`buttonRecipe`, `buttonParts`). */
  readonly recipe: { readonly map: string; readonly parts: string };
  readonly anatomy: readonly AnatomyPart[];
  readonly accessibility: AccessibilityContract;
  readonly guidance: { readonly do: readonly string[]; readonly dont: readonly string[] };
  /** Named exports of `<Name>.examples.tsx`, in reading order. The first is the hero example. */
  readonly examples: readonly ExampleRef[];
  /** What the component cannot do yet, the primitive capability that unblocks it, and today's form. */
  readonly gaps: readonly Gap[];
  readonly related?: readonly string[];
  /** Optional translations of the prose fields, keyed by BCP 47 tag (`zh-Hans`). */
  readonly translations?: Readonly<Record<string, Partial<Pick<ComponentDoc, 'summary' | 'description' | 'guidance'>>>>;
}

export interface AnatomyPart {
  /** A recipe part name, so the reference can show the classes it carries. */
  readonly part: string;
  readonly required: boolean;
  readonly description: string;
}

export interface AccessibilityContract {
  /** The pattern this component binds to; its checks run in the gallery specs. `null` for layout
   * that holds a control but is not one (Field). */
  readonly pattern: PatternName | null;
  /** Where the accessible name comes from. */
  readonly name: 'label' | 'label then description' | 'label, then disabledMessage when disabled';
  /** States the component exposes and the props that drive them. */
  readonly states: readonly { readonly state: StateName; readonly from: string }[];
  /** Keys beyond the pattern's own, or `[]`. */
  readonly keyboard: readonly { readonly keys: string; readonly effect: string }[];
  /** Smallest press target in logical pixels at each size, measured by the gallery spec. */
  readonly target?: Readonly<Partial<Record<'sm' | 'md' | 'lg', number>>>;
  /** Contrast pairs the theme pack must prove, as role pairs (`on-accent` on `accent`). */
  readonly contrast: readonly ContrastPair[];
  /** Plain-language notes for the reference; keep them short. */
  readonly notes?: readonly string[];
}

export interface ContrastPair {
  readonly what: string;
  /** Foreground role, then background role. */
  readonly pair: readonly [string, string];
  readonly minimum: '3:1' | '4.5:1';
  /** Whether the theme pack proves this pair today; an unproven pair is a theme gap. */
  readonly proven: boolean;
}

export type StateName = 'disabled' | 'busy' | 'checked' | 'expanded' | 'selected' | 'pressed' | 'modal';

export interface ExampleRef {
  /** Named export of the examples module. */
  readonly name: string;
  readonly title: string;
  readonly description?: string;
}

export interface Gap {
  /** What the component cannot do. */
  readonly missing: string;
  /** The primitive capability it waits for, by the name the primitive cleanup uses. */
  readonly needs: string;
  /** What ships meanwhile. */
  readonly today: string;
}
