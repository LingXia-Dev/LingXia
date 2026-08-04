/**
 * Light/dark design tokens for the showcase lxapp.
 *
 * Every colour the pages use is compiled to `rgb(var(--lx-…) / <alpha-value>)`
 * instead of a literal, and the variables are re-declared under
 * `[data-theme="dark"]`. The runtime stamps `data-theme` on `<html>` whenever the
 * resolved appearance changes, so the whole app flips without any page touching
 * a `dark:` variant. `prefers-color-scheme` is only a first-paint fallback:
 * platform media queries can lag an in-place appearance switch, so
 * `[data-theme]` always wins.
 *
 * A token can resolve to a different value per *role*, because the same
 * Tailwind shade means different things as a background, a label, or a hairline
 * (`bg-white` is a card and must go dark; `text-white` sits on an accent fill
 * and must stay white). Roles: `bg` (also rings/gradients/placeholders' source),
 * `fg` (text), `bd` (borders/dividers).
 */

import defaultColors from "tailwindcss/colors";

/** Accent ramps taken from Tailwind's defaults. */
const ACCENT_HUES = [
  "slate",
  "red",
  "orange",
  "amber",
  "yellow",
  "lime",
  "green",
  "emerald",
  "teal",
  "cyan",
  "sky",
  "blue",
  "indigo",
  "violet",
  "purple",
  "fuchsia",
  "pink",
  "rose",
];

const ACCENT_SHADES = [50, 100, 200, 300, 400, 500, 600, 700, 800, 900];

/**
 * Dark-mode shade remap for accent *fills*: the pale tints used as panels
 * (`bg-blue-50`, `border-red-200`) invert into deep tints, while the saturated
 * mid-tones that carry white text (`bg-blue-500`) keep their identity so brand
 * fills and contrast survive the flip.
 */
const DARK_FILL_SHADE = { 50: 950, 100: 900, 200: 800, 300: 700 };

/**
 * Dark-mode shade remap for accent *text*: pale shades already sit on a deep
 * fill and stay put; the readable mid/dark shades lighten.
 */
const DARK_TEXT_SHADE = { 400: 300, 500: 400, 600: 400, 700: 300, 800: 300, 900: 200 };

const GRAY_SHADES = [50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950];

/** iOS system grays — the light branch, unchanged from the original config. */
const LIGHT_GRAY = {
  50: "#F2F2F7",
  100: "#E5E5EA",
  200: "#D1D1D6",
  300: "#C7C7CC",
  400: "#AEAEB2",
  500: "#8E8E93",
  600: "#636366",
  700: "#48484A",
  800: "#3A3A3C",
  900: "#2C2C2E",
  950: "#1C1C1E",
};

/**
 * Dark fills climb 50→500 (page → sunken → raised surfaces, matching how the
 * pages use them) and then fall again for 600→950, which are contrast chips
 * holding light text and must stay dark rather than invert.
 */
const DARK_GRAY_BG = {
  50: "#17171A",
  100: "#2A2A2E",
  200: "#333338",
  300: "#3E3E44",
  400: "#55555C",
  500: "#6B6B73",
  600: "#4A4A50",
  700: "#3A3A40",
  800: "#2E2E33",
  900: "#232327",
  950: "#17171A",
};

/** Dark labels: 400 tertiary → 900 primary, plus the already-light 50…300. */
const DARK_GRAY_FG = {
  50: "#FAFAFC",
  100: "#F2F2F7",
  200: "#E5E5EA",
  300: "#D1D1D6",
  400: "#7A7A82",
  500: "#98989F",
  600: "#B0B0B8",
  700: "#C9C9D0",
  800: "#DEDEE4",
  900: "#F2F2F7",
  950: "#FAFAFC",
};

/** Dark hairlines brighten monotonically with the shade index. */
const DARK_GRAY_BD = {
  50: "#232327",
  100: "#2A2A2E",
  200: "#333338",
  300: "#3E3E44",
  400: "#4A4A50",
  500: "#5A5A62",
  600: "#6B6B73",
  700: "#7A7A82",
  800: "#8A8A92",
  900: "#9A9AA2",
  950: "#AAAAB2",
};

/** Tokens whose value is the same in every role. */
const SINGLE_ROLE_TOKENS = {
  primary: { light: "#007AFF", dark: "#0A84FF" },
  "primary-light": { light: "#5AC8FA", dark: "#64D2FF" },
  "primary-dark": { light: "#0066CC", dark: "#0A84FF" },
  "primary-foreground": { light: "#FFFFFF", dark: "#FFFFFF" },
  success: { light: "#34C759", dark: "#30D158" },
  warning: { light: "#FF9500", dark: "#FF9F0A" },
  error: { light: "#FF3B30", dark: "#FF453A" },
  danger: { light: "#FF3B30", dark: "#FF453A" },
  info: { light: "#5AC8FA", dark: "#64D2FF" },
  // shadcn-shaped semantic tokens: the shared components in `shared/components`
  // are written against these names.
  background: { light: "#F5F5F7", dark: "#17171A" },
  foreground: { light: "#1D1D1F", dark: "#F2F2F7" },
  card: { light: "#FFFFFF", dark: "#1F1F22" },
  "card-foreground": { light: "#1D1D1F", dark: "#F2F2F7" },
  popover: { light: "#FFFFFF", dark: "#1F1F22" },
  "popover-foreground": { light: "#1D1D1F", dark: "#F2F2F7" },
  muted: { light: "#F2F2F7", dark: "#2A2A2E" },
  "muted-foreground": { light: "#6B6B70", dark: "#98989F" },
  accent: { light: "#F2F2F7", dark: "#2A2A2E" },
  "accent-foreground": { light: "#1D1D1F", dark: "#F2F2F7" },
  secondary: { light: "#E5E5EA", dark: "#2A2A2E" },
  "secondary-foreground": { light: "#1D1D1F", dark: "#F2F2F7" },
  destructive: { light: "#FF3B30", dark: "#FF453A" },
  "destructive-foreground": { light: "#FFFFFF", dark: "#FFFFFF" },
  border: { light: "#E5E5EA", dark: "#333338" },
  input: { light: "#E5E5EA", dark: "#3E3E44" },
  ring: { light: "#007AFF", dark: "#0A84FF" },
};

/** Box-shadow colours: dark surfaces need a heavier scrim to read as elevated. */
const SHADOW_TOKENS = {
  "shadow-sm": { light: "0 0 0 / 0.05", dark: "0 0 0 / 0.35" },
  shadow: { light: "0 0 0 / 0.1", dark: "0 0 0 / 0.5" },
  "shadow-md": { light: "0 0 0 / 0.15", dark: "0 0 0 / 0.55" },
  "shadow-lg": { light: "0 0 0 / 0.2", dark: "0 0 0 / 0.6" },
  "shadow-xl": { light: "0 0 0 / 0.25", dark: "0 0 0 / 0.65" },
};

function channels(hex) {
  const value = hex.replace("#", "");
  const full =
    value.length === 3
      ? value
          .split("")
          .map((c) => c + c)
          .join("")
      : value;
  const int = parseInt(full.slice(0, 6), 16);
  return `${(int >> 16) & 255} ${(int >> 8) & 255} ${int & 255}`;
}

/** `{ [tokenName]: { light, dark } }` per role, as raw hex. */
function buildTokens() {
  const bg = {};
  const fg = {};
  const bd = {};

  for (const hue of ACCENT_HUES) {
    const ramp = defaultColors[hue];
    for (const shade of ACCENT_SHADES) {
      const name = `${hue}-${shade}`;
      bg[name] = { light: ramp[shade], dark: ramp[DARK_FILL_SHADE[shade] ?? shade] };
      fg[name] = { light: ramp[shade], dark: ramp[DARK_TEXT_SHADE[shade] ?? shade] };
      // Accent hairlines follow the fill remap; they reuse the `bg` variables.
    }
  }

  for (const shade of GRAY_SHADES) {
    const name = `gray-${shade}`;
    bg[name] = { light: LIGHT_GRAY[shade], dark: DARK_GRAY_BG[shade] };
    fg[name] = { light: LIGHT_GRAY[shade], dark: DARK_GRAY_FG[shade] };
    bd[name] = { light: LIGHT_GRAY[shade], dark: DARK_GRAY_BD[shade] };
  }

  bg.white = { light: "#FFFFFF", dark: "#1F1F22" };
  // Scrims (`bg-black/40` over media) stay black in both branches.
  bg.black = { light: "#000000", dark: "#000000" };
  fg.white = { light: "#FFFFFF", dark: "#FFFFFF" };
  fg.black = { light: "#000000", dark: "#F2F2F7" };
  bd.white = { light: "#FFFFFF", dark: "#333338" };
  bd.black = { light: "#000000", dark: "#000000" };

  return { bg, fg, bd };
}

const TOKENS = buildTokens();

function ref(role, name) {
  return `rgb(var(--lx-${role}-${name}) / <alpha-value>)`;
}

/** The `single` role resolves to the same variable whatever the utility. */
function singleRef(name) {
  return `rgb(var(--lx-c-${name}) / <alpha-value>)`;
}

function semanticColors() {
  const colors = {};
  const nest = (key, value) => {
    const dash = key.lastIndexOf("-");
    if (dash === -1) {
      colors[key] = typeof colors[key] === "object" ? { ...colors[key], DEFAULT: value } : value;
      return;
    }
    const parent = key.slice(0, dash);
    const child = key.slice(dash + 1);
    const existing = colors[parent];
    const bucket = typeof existing === "object" ? existing : existing ? { DEFAULT: existing } : {};
    bucket[child] = value;
    colors[parent] = bucket;
  };
  for (const name of Object.keys(SINGLE_ROLE_TOKENS)) nest(name, singleRef(name));
  return colors;
}

/**
 * Palette for one Tailwind colour role. Accent hues have no dedicated border
 * variables, so the `bd` role falls back to the fill ones.
 */
function palette(role) {
  const varRole = role === "bd" ? "bg" : role;
  const colors = {};

  for (const hue of ACCENT_HUES) {
    colors[hue] = {};
    for (const shade of ACCENT_SHADES) {
      colors[hue][shade] = ref(varRole, `${hue}-${shade}`);
    }
  }

  colors.gray = {};
  for (const shade of GRAY_SHADES) colors.gray[shade] = ref(role, `gray-${shade}`);

  colors.white = ref(role, "white");
  colors.black = ref(role, "black");

  return { ...colors, ...semanticColors() };
}

export const backgroundPalette = palette("bg");
export const textPalette = palette("fg");
export const borderPalette = palette("bd");

export const boxShadow = {
  sm: "0 1px 2px 0 var(--lx-shadow-sm)",
  DEFAULT: "0 2px 8px 0 var(--lx-shadow)",
  md: "0 4px 12px 0 var(--lx-shadow-md)",
  lg: "0 8px 16px 0 var(--lx-shadow-lg)",
  xl: "0 12px 24px 0 var(--lx-shadow-xl)",
};

function variables(branch) {
  const declarations = {};
  for (const [role, tokens] of Object.entries(TOKENS)) {
    for (const [name, value] of Object.entries(tokens)) {
      declarations[`--lx-${role}-${name}`] = channels(value[branch]);
    }
  }
  for (const [name, value] of Object.entries(SINGLE_ROLE_TOKENS)) {
    declarations[`--lx-c-${name}`] = channels(value[branch]);
  }
  for (const [name, value] of Object.entries(SHADOW_TOKENS)) {
    declarations[`--lx-${name}`] = `rgb(${value[branch]})`;
  }
  return declarations;
}

/**
 * Readable aliases for hand-written CSS (page stylesheets, scoped Vue styles).
 * They point at the role variables, so they flip with the rest of the theme and
 * are declared only once.
 */
const ALIASES = {
  "--lx-page": "rgb(var(--lx-bg-gray-50))",
  "--lx-surface": "rgb(var(--lx-bg-white))",
  "--lx-surface-raised": "rgb(var(--lx-bg-gray-100))",
  "--lx-text": "rgb(var(--lx-fg-gray-900))",
  "--lx-text-secondary": "rgb(var(--lx-fg-gray-500))",
  "--lx-text-tertiary": "rgb(var(--lx-fg-gray-400))",
  "--lx-border": "rgb(var(--lx-bd-gray-200))",
  "--lx-border-subtle": "rgb(var(--lx-bd-gray-100))",
  "--lx-accent": "rgb(var(--lx-c-primary))",
};

export const baseStyles = {
  ":root": { colorScheme: "light", ...variables("light"), ...ALIASES },
  "@media (prefers-color-scheme: dark)": {
    // Guarded so an explicitly light lxapp is not dragged dark by the OS.
    ':root:not([data-theme="light"])': { colorScheme: "dark", ...variables("dark") },
  },
  '[data-theme="dark"]': { colorScheme: "dark", ...variables("dark") },
};
