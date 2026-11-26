// Generated from contracts/view/palette.json; do not edit.
import type { ColorRole } from './authoring.js';
/** A palette slot: a role, or a line pair a lifted surface draws its contents with. */
export type PaletteSlot = ColorRole | "line-on-raised" | "line-strong-on-raised" | "line-on-overlay" | "line-strong-on-overlay" | "danger-on-raised" | "danger-on-overlay";
/** The default theme resolved per look, high contrast included, derived roles too: slot → #rrggbbaa. */
export declare const defaultPalette: { readonly [look in "light" | "dark" | "high-contrast-light" | "high-contrast-dark"]: Readonly<Record<PaletteSlot, string>> };
