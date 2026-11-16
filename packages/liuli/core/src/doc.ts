// The shape of each component's `X.doc.ts`: what the code cannot say. Props come from the exported
// props type and its JSDoc (`@default` included), classes from the recipe, so neither is repeated here.

import type { PatternName } from './a11y/patterns';

/** The gallery's sections, in the order the home page shows them. */
export const categories = ['actions', 'inputs', 'selection', 'navigation', 'feedback', 'overlays', 'content', 'charts', 'media'] as const;
export type Category = (typeof categories)[number];

export interface ComponentDoc {
  /** Component name, equal to its export and its folder. */
  readonly name: string;
  readonly category: Category;
  /** `preview` may still change; `stable` once its accessibility contract passes on all four platforms. */
  readonly status: 'stable' | 'preview';
  /** The day it joined the package, `YYYY-MM-DD`; the gallery's "New" list reads it. */
  readonly since: string;
  /** One line for the catalog and the skill index; no trailing period. */
  readonly summary: string;
  /** What it is for, in two or three sentences. */
  readonly description: string;
  /** Search terms an agent or a person might use instead of the name. */
  readonly keywords: readonly string[];
  /** The exported props type the generator reads (`ButtonProps`). */
  readonly propsType: string;
  /** Props types of the other components the same import exports (`DropdownMenuItemProps`). */
  readonly otherPropsTypes?: readonly string[];
  /** The exported recipe and its part manifest (`buttonRecipe`, `buttonParts`). */
  readonly recipe: { readonly map: string; readonly parts: string };
  readonly anatomy: readonly AnatomyPart[];
  readonly accessibility: AccessibilityContract;
  readonly guidance: { readonly do: readonly string[]; readonly dont: readonly string[] };
  /** Named exports of `<Name>.examples.tsx`, in reading order. The first is the hero example. */
  readonly examples: readonly ExampleRef[];
  /** What the component cannot do yet, the primitive capability that unblocks it, and today's form. */
  readonly gaps: readonly Gap[];
  /** What it does not do and is not waiting on anything for: a decision, or work nobody has asked for. One sentence each. */
  readonly limits?: readonly string[];
  readonly related?: readonly string[];
  /** What the gallery shows, by example export name. */
  readonly gallery: GalleryDoc;
  /** Puts the component in the gallery's playground, starting from these props. Only for components that draw from plain values: no handlers, no children. */
  readonly playground?: { readonly props: Readonly<Record<string, string | number | boolean>> };
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
   * that holds a control but is not one (Field), or content that is not a control (Avatar). */
  readonly pattern: PatternName | null;
  /** The prop the accessible name comes from, one of the component's own (`label`, `name`, `alt`, `aria-label`); `null` for a live region read by its content (Banner). */
  readonly name: string | null;
  /** What is read after the name, through `aria-describedby`, in order; `[]` when nothing is. */
  readonly description: readonly string[];
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
  /** Foreground role, then background role; `role/NN` paints the role at NN% opacity. */
  readonly pair: readonly [string, string];
  /** The role under a translucent background. */
  readonly over?: string;
  /** The ratio contrast.test.mjs computes from the default theme, light and dark, must reach. */
  readonly minimum: '3:1' | '4.5:1';
}

export type StateName = 'disabled' | 'busy' | 'checked' | 'expanded' | 'selected' | 'pressed' | 'modal' | 'invalid' | 'required';

export interface ExampleRef {
  /** Named export of the examples module. */
  readonly name: string;
  readonly title: string;
  readonly description?: string;
}

export interface GalleryDoc {
  /** A small, still specimen for the home tile; it is drawn on a 96 px stage. */
  readonly specimen: string;
  /** The tile's line: at most 19 characters, so it fits one line of a half-width tile in any system face; no trailing period. */
  readonly tagline: string;
  /** Position within its category on the home page and in the manifest, from 1. */
  readonly order: number;
  /** The component page; until set, the page lists `examples` in order. */
  readonly page?: {
    /** One sentence on when to use it. */
    readonly lead: string;
    /** `bare` puts examples straight on the page, for containers that draw their own surface. @default 'framed' */
    readonly stage?: 'framed' | 'bare';
    readonly hero: string;
    /** A still drawing of a component that is hard to see at rest (an open dialog), on its own bare stage. */
    readonly parts?: ExampleRef;
    readonly variants?: readonly ExampleRef[];
    readonly sizes?: readonly ExampleRef[];
    /** Each state drawn on its own stage, forced by props rather than by interaction. */
    readonly states?: readonly ExampleRef[];
    /** The states section's heading, when the states are of one kind: "Loading, empty and error". @default 'States' */
    readonly statesTitle?: string;
    /** One sentence under that heading; by default it says the states are staged. */
    readonly statesNote?: string;
    /** `stack` gives each state the full width on a phone, for a field whose value has a fixed length. @default 'grid' */
    readonly statesLayout?: 'grid' | 'stack';
    readonly inContext?: readonly ExampleRef[];
  };
}

export interface Gap {
  /** What the component cannot do. */
  readonly missing: string;
  /** The primitive capability it waits for, by the name the primitive cleanup uses. */
  readonly needs: string;
  /** What ships meanwhile. */
  readonly today: string;
}
