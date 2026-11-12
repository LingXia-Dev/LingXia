// Generated from contracts/view/palette.json; do not edit.
export type Role = "canvas" | "surface" | "surface-raised" | "surface-sunken" | "line" | "text" | "text-muted" | "text-faint" | "accent" | "on-accent" | "accent-soft" | "on-accent-soft" | "danger" | "on-danger" | "danger-soft" | "success" | "on-success" | "success-soft" | "warning" | "on-warning" | "warning-soft" | "focus" | "scrim" | "chart-1" | "chart-2" | "chart-3" | "chart-4" | "chart-5" | "line-strong" | "selection" | "shadow" | "media" | "on-media";
/** The default theme pack resolved per appearance, derived roles included: role → #rrggbbaa. */
export declare const defaultPalette: { readonly light: Readonly<Record<Role, string>>; readonly dark: Readonly<Record<Role, string>> };
