// Generated from the doc file by scripts/generate.mjs; do not edit.

export interface GalleryEntry { name: string; page: string; category: string; summary: string; keywords: string[]; since: string; status: string }
export interface GalleryCategory { id: string; title: string; description: string; page: string; count: number }

export const entries: GalleryEntry[] = [
  {
    "name": "Colour",
    "page": "colour",
    "category": "foundations",
    "summary": "The theme's colour roles and what each is for",
    "keywords": [],
    "since": "",
    "status": "preview"
  },
  {
    "name": "Typography",
    "page": "typography",
    "category": "foundations",
    "summary": "Six type sizes, their line heights and weights",
    "keywords": [],
    "since": "",
    "status": "preview"
  },
  {
    "name": "Shape & spacing",
    "page": "shape-and-spacing",
    "category": "foundations",
    "summary": "Corner radii, control heights, spacing steps and focus shapes",
    "keywords": [],
    "since": "",
    "status": "preview"
  },
  {
    "name": "Elevation",
    "page": "elevation",
    "category": "foundations",
    "summary": "Flat, resting, raised and modal surfaces",
    "keywords": [],
    "since": "",
    "status": "preview"
  },
  {
    "name": "Motion",
    "page": "motion",
    "category": "foundations",
    "summary": "How things enter, move and respond, and reduced motion",
    "keywords": [],
    "since": "",
    "status": "preview"
  },
  {
    "name": "Button",
    "page": "button",
    "category": "actions",
    "summary": "Runs an action when pressed, with variants, sizes and a loading state",
    "keywords": [
      "button",
      "action",
      "submit",
      "cta",
      "primary",
      "destructive",
      "loading",
      "press",
      "tap"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "ToggleButton",
    "page": "toggle-button",
    "category": "actions",
    "summary": "A button that turns one option on or off and stays down while on",
    "keywords": [
      "toggle button",
      "toggle",
      "pressed",
      "filter",
      "chip",
      "on off",
      "option",
      "button"
    ],
    "since": "2026-11-13",
    "status": "preview"
  },
  {
    "name": "IconButton",
    "page": "icon-button",
    "category": "actions",
    "summary": "A button that shows only an icon, with a name that is always read",
    "keywords": [
      "icon button",
      "toolbar button",
      "action icon",
      "square button",
      "close button",
      "more button"
    ],
    "since": "2026-11-14",
    "status": "preview"
  },
  {
    "name": "ButtonGroup",
    "page": "button-group",
    "category": "actions",
    "summary": "Buttons that belong together, apart or joined edge to edge, as one keyboard stop",
    "keywords": [
      "button group",
      "toolbar",
      "action group",
      "attached buttons",
      "split actions",
      "button bar"
    ],
    "since": "2026-11-14",
    "status": "preview"
  },
  {
    "name": "TextInput",
    "page": "text-input",
    "category": "inputs",
    "summary": "Single-line text field with label, help text, status and an optional clear button",
    "keywords": [
      "text input",
      "text field",
      "input",
      "textbox",
      "email",
      "password",
      "search",
      "form"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "TextArea",
    "page": "text-area",
    "category": "inputs",
    "summary": "Multi-line text field that grows with its text, with label, help text and status",
    "keywords": [
      "textarea",
      "text area",
      "multiline",
      "multi-line",
      "note",
      "comment",
      "reply",
      "message",
      "description",
      "form"
    ],
    "since": "2026-11-13",
    "status": "preview"
  },
  {
    "name": "DateInput",
    "page": "date-input",
    "category": "inputs",
    "summary": "A labelled date field that opens a month picker",
    "keywords": [
      "date input",
      "date picker",
      "calendar",
      "day",
      "due date",
      "birthday",
      "form",
      "field"
    ],
    "since": "2026-11-13",
    "status": "preview"
  },
  {
    "name": "TimeInput",
    "page": "time-input",
    "category": "inputs",
    "summary": "A labelled time field that opens an hour and minute picker",
    "keywords": [
      "time input",
      "time picker",
      "clock",
      "hour",
      "minute",
      "reminder",
      "alarm",
      "form",
      "field"
    ],
    "since": "2026-11-13",
    "status": "preview"
  },
  {
    "name": "Slider",
    "page": "slider",
    "category": "inputs",
    "summary": "A value picked along a track, with its label and the value in words",
    "keywords": [
      "slider",
      "range",
      "scrubber",
      "volume",
      "seek",
      "position",
      "track",
      "thumb"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "Field",
    "page": "field",
    "category": "inputs",
    "summary": "Label, help text and validation status around one form control",
    "keywords": [
      "field",
      "form",
      "label",
      "help text",
      "error",
      "validation",
      "required"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "SearchField",
    "page": "search-field",
    "category": "inputs",
    "summary": "A field for searching what is on the page, with a clear button and a line that says what was found",
    "keywords": [
      "search",
      "search field",
      "search box",
      "filter",
      "find",
      "query",
      "magnifier"
    ],
    "since": "2026-11-15",
    "status": "preview"
  },
  {
    "name": "NumberInput",
    "page": "number-input",
    "category": "inputs",
    "summary": "A number typed or nudged by steps, kept between a minimum and a maximum",
    "keywords": [
      "number",
      "number input",
      "stepper",
      "quantity",
      "counter",
      "spinbutton",
      "amount",
      "numeric"
    ],
    "since": "2026-11-15",
    "status": "preview"
  },
  {
    "name": "CheckboxInput",
    "page": "checkbox-input",
    "category": "selection",
    "summary": "One on/off choice with a label, help text and status",
    "keywords": [
      "checkbox",
      "check",
      "tick",
      "opt in",
      "agree",
      "terms",
      "toggle",
      "boolean"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "RadioGroup",
    "page": "radio-group",
    "category": "selection",
    "summary": "One choice from a short list, every option in view",
    "keywords": [
      "radio",
      "radio group",
      "radio button",
      "option",
      "choice",
      "plan",
      "pick one",
      "single select"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "Switch",
    "page": "switch",
    "category": "selection",
    "summary": "A setting that turns on or off the moment it flips",
    "keywords": [
      "switch",
      "toggle",
      "on off",
      "setting",
      "preference",
      "enable",
      "disable"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "Select",
    "page": "select",
    "category": "selection",
    "summary": "One choice from a list that opens on demand",
    "keywords": [
      "select",
      "dropdown",
      "picker",
      "combo box",
      "choose",
      "options",
      "menu",
      "single select"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "SegmentedControl",
    "page": "segmented-control",
    "category": "selection",
    "summary": "One value from a few, as a row of segments with a thumb on the chosen one",
    "keywords": [
      "segmented control",
      "segments",
      "radio",
      "choice",
      "switcher",
      "units",
      "mode",
      "picker"
    ],
    "since": "2026-11-13",
    "status": "preview"
  },
  {
    "name": "Combobox",
    "page": "combobox",
    "category": "selection",
    "summary": "One choice from a long list, found by typing: in the field itself with a keyboard, in a sheet with a finger",
    "keywords": [
      "combobox",
      "autocomplete",
      "typeahead",
      "searchable select",
      "picker",
      "filter",
      "search select",
      "lookup"
    ],
    "since": "2026-11-15",
    "status": "preview"
  },
  {
    "name": "CheckboxList",
    "page": "checkbox-list",
    "category": "selection",
    "summary": "Several on/off choices under one question, any number of which can be chosen",
    "keywords": [
      "checkbox list",
      "checkbox group",
      "multiple choice",
      "multi select",
      "options",
      "checklist"
    ],
    "since": "2026-11-15",
    "status": "preview"
  },
  {
    "name": "Tabs",
    "page": "tabs",
    "category": "navigation",
    "summary": "A row of tabs over one panel, as an underline or a segmented control",
    "keywords": [
      "tabs",
      "tab bar",
      "segmented control",
      "segments",
      "switcher",
      "sections",
      "filter",
      "range"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "NavList",
    "page": "nav-list",
    "category": "navigation",
    "summary": "A sidebar's list of places, in sections that open and close, with the current one marked",
    "keywords": [
      "nav list",
      "navigation list",
      "sidebar",
      "side bar",
      "source list",
      "outline",
      "menu",
      "table of contents",
      "tree"
    ],
    "since": "2026-11-14",
    "status": "preview"
  },
  {
    "name": "Badge",
    "page": "badge",
    "category": "feedback",
    "summary": "A short status or category label in a tinted pill",
    "keywords": [
      "badge",
      "tag",
      "label",
      "pill",
      "chip",
      "status",
      "lozenge",
      "tone"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "ProgressBar",
    "page": "progress-bar",
    "category": "feedback",
    "summary": "How much of a task is done, or a spinner while the amount is unknown",
    "keywords": [
      "progress",
      "progressbar",
      "loading",
      "spinner",
      "busy",
      "upload",
      "download",
      "meter",
      "indeterminate"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "Banner",
    "page": "banner",
    "category": "feedback",
    "summary": "A message in the page about the page or the account, that stays until resolved",
    "keywords": [
      "banner",
      "alert",
      "callout",
      "notice",
      "message",
      "status",
      "warning",
      "error",
      "inline"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "EmptyState",
    "page": "empty-state",
    "category": "feedback",
    "summary": "What a page or list shows when it has nothing in it yet, and the way forward",
    "keywords": [
      "empty state",
      "empty",
      "blank slate",
      "no results",
      "nothing here",
      "zero state",
      "first run",
      "placeholder"
    ],
    "since": "2026-11-13",
    "status": "preview"
  },
  {
    "name": "Skeleton",
    "page": "skeleton",
    "category": "feedback",
    "summary": "Quiet shapes that stand in for content while it loads",
    "keywords": [
      "skeleton",
      "placeholder",
      "loading",
      "shimmer",
      "ghost",
      "content loader",
      "busy"
    ],
    "since": "2026-11-13",
    "status": "preview"
  },
  {
    "name": "Toast",
    "page": "toast",
    "category": "feedback",
    "summary": "A short message about something that just happened, over the bottom of the screen",
    "keywords": [
      "toast",
      "snackbar",
      "notification",
      "undo",
      "message",
      "confirmation",
      "feedback",
      "status"
    ],
    "since": "2026-11-13",
    "status": "preview"
  },
  {
    "name": "Dialog",
    "page": "dialog",
    "category": "overlays",
    "summary": "Modal panel over the page for a decision or a short task",
    "keywords": [
      "dialog",
      "modal",
      "confirm",
      "alert",
      "popup",
      "overlay",
      "prompt"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "DropdownMenu",
    "page": "dropdown-menu",
    "category": "overlays",
    "summary": "Button that opens a short menu of actions",
    "keywords": [
      "dropdown",
      "menu",
      "more",
      "overflow",
      "kebab",
      "actions",
      "popover",
      "context"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "Tooltip",
    "page": "tooltip",
    "category": "overlays",
    "summary": "An info button that opens a short note about what is beside it",
    "keywords": [
      "tooltip",
      "toggletip",
      "info",
      "help",
      "hint",
      "explain",
      "popover",
      "note"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "BottomSheet",
    "page": "bottom-sheet",
    "category": "overlays",
    "summary": "A modal panel on the bottom edge that drags between detents and off to close",
    "keywords": [
      "bottom sheet",
      "sheet",
      "drawer",
      "action sheet",
      "detents",
      "modal",
      "panel",
      "overlay"
    ],
    "since": "2026-11-13",
    "status": "preview"
  },
  {
    "name": "Popover",
    "page": "popover",
    "category": "overlays",
    "summary": "A button that opens a panel of content above or below it, closed by a tap outside",
    "keywords": [
      "popover",
      "popup",
      "panel",
      "flyout",
      "options",
      "share",
      "overlay",
      "anchored"
    ],
    "since": "2026-11-13",
    "status": "preview"
  },
  {
    "name": "Card",
    "page": "card",
    "category": "content",
    "summary": "A surface that groups related content, or one pressable choice",
    "keywords": [
      "card",
      "panel",
      "tile",
      "surface",
      "container",
      "box",
      "clickable card",
      "option card",
      "choice"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "Item",
    "page": "item",
    "category": "content",
    "summary": "One row of a list: a label, a second line, and content at either end",
    "keywords": [
      "item",
      "row",
      "list",
      "cell",
      "list item",
      "settings row",
      "table row",
      "navigation row"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "Details",
    "page": "details",
    "category": "content",
    "summary": "A summary row that shows and hides the content under it",
    "keywords": [
      "details",
      "disclosure",
      "accordion",
      "expand",
      "collapse",
      "faq",
      "show more",
      "summary"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "Avatar",
    "page": "avatar",
    "category": "content",
    "summary": "A person or team as a circle: their picture, or their initials",
    "keywords": [
      "avatar",
      "profile picture",
      "photo",
      "initials",
      "user",
      "person",
      "member",
      "facepile"
    ],
    "since": "2026-11-12",
    "status": "preview"
  },
  {
    "name": "SplitView",
    "page": "split-view",
    "category": "content",
    "summary": "A sidebar beside its content for a wide window, each scrolling on its own",
    "keywords": [
      "split view",
      "sidebar layout",
      "master detail",
      "two pane",
      "workspace",
      "navigation split",
      "app shell"
    ],
    "since": "2026-11-14",
    "status": "preview"
  },
  {
    "name": "Icon",
    "page": "icon",
    "category": "content",
    "summary": "An icon by what it means, from a set an app or theme can replace",
    "keywords": [
      "icon",
      "glyph",
      "symbol",
      "pictogram",
      "lucide",
      "icon set",
      "vector"
    ],
    "since": "2026-11-14",
    "status": "preview"
  },
  {
    "name": "Table",
    "page": "table",
    "category": "content",
    "summary": "Records in rows and fields in columns, under headings that stay in view",
    "keywords": [
      "table",
      "data table",
      "grid",
      "rows",
      "columns",
      "records",
      "spreadsheet",
      "data display"
    ],
    "since": "2026-11-15",
    "status": "preview"
  },
  {
    "name": "Markdown",
    "page": "markdown",
    "category": "content",
    "summary": "Markdown text drawn as Liuli type: a message, a note, a help page",
    "keywords": [
      "markdown",
      "rich text",
      "prose",
      "formatted text",
      "viewer",
      "message",
      "article",
      "help text"
    ],
    "since": "2026-11-15",
    "status": "preview"
  },
  {
    "name": "Stat",
    "page": "stat",
    "category": "charts",
    "summary": "One figure with its label, how it changed, and an optional small chart",
    "keywords": [
      "stat",
      "statistic",
      "metric",
      "kpi",
      "figure",
      "number",
      "delta",
      "change",
      "trend",
      "dashboard",
      "tile"
    ],
    "since": "2026-11-14",
    "status": "preview"
  },
  {
    "name": "Sparkline",
    "page": "sparkline",
    "category": "charts",
    "summary": "A small chart without axes: the shape of a series as a line, an area or bars",
    "keywords": [
      "sparkline",
      "trend",
      "mini chart",
      "inline chart",
      "line",
      "area",
      "bar",
      "series",
      "history"
    ],
    "since": "2026-11-14",
    "status": "preview"
  },
  {
    "name": "BarChart",
    "page": "bar-chart",
    "category": "charts",
    "summary": "Amounts by category, as bars on a zero line, with selection and grouped series",
    "keywords": [
      "bar chart",
      "column chart",
      "bars",
      "histogram",
      "categories",
      "compare",
      "grouped bars",
      "chart",
      "graph"
    ],
    "since": "2026-11-14",
    "status": "preview"
  },
  {
    "name": "LineChart",
    "page": "line-chart",
    "category": "charts",
    "summary": "A number over time, as a line that fits its data or an area from zero, for up to five series",
    "keywords": [
      "line chart",
      "area chart",
      "trend",
      "time series",
      "series",
      "history",
      "chart",
      "graph"
    ],
    "since": "2026-11-14",
    "status": "preview"
  },
  {
    "name": "PieChart",
    "page": "pie-chart",
    "category": "charts",
    "summary": "Parts of one whole, as a ring or a solid pie, listed with their amounts in a legend that selects",
    "keywords": [
      "pie chart",
      "donut",
      "doughnut",
      "ring",
      "share",
      "proportion",
      "parts of a whole",
      "chart",
      "graph"
    ],
    "since": "2026-11-14",
    "status": "preview"
  },
  {
    "name": "Chart",
    "page": "chart",
    "category": "charts",
    "summary": "The frame every chart sits in: heading, plot, legend, readout and the loading, empty and error states",
    "keywords": [
      "chart",
      "frame",
      "legend",
      "readout",
      "plot",
      "graph",
      "data visualisation",
      "empty",
      "loading"
    ],
    "since": "2026-11-14",
    "status": "preview"
  },
  {
    "name": "VideoPlayer",
    "page": "video-player",
    "category": "media",
    "summary": "A video with its controls, over the picture or under it, and full screen",
    "keywords": [
      "video",
      "player",
      "movie",
      "stream",
      "hls",
      "media",
      "playback",
      "fullscreen"
    ],
    "since": "2026-11-13",
    "status": "preview"
  },
  {
    "name": "VideoControls",
    "page": "video-controls",
    "category": "media",
    "summary": "Play, seek, mute, speed, quality and full screen for a useVideo player",
    "keywords": [
      "video",
      "player",
      "controls",
      "scrubber",
      "seek",
      "media",
      "playback",
      "fullscreen"
    ],
    "since": "2026-11-13",
    "status": "preview"
  },
  {
    "name": "Sign in",
    "page": "sign-in",
    "category": "patterns",
    "summary": "A sign-in form that validates and reports errors",
    "keywords": [],
    "since": "",
    "status": "preview"
  },
  {
    "name": "Settings",
    "page": "settings",
    "category": "patterns",
    "summary": "A settings screen of rows, switches and a confirmation",
    "keywords": [],
    "since": "",
    "status": "preview"
  },
  {
    "name": "Uploads",
    "page": "uploads",
    "category": "patterns",
    "summary": "Long-running work with progress, failure and retry",
    "keywords": [],
    "since": "",
    "status": "preview"
  }
];

export const galleryCategories: GalleryCategory[] = [
  {
    "id": "foundations",
    "title": "Foundations",
    "description": "The colour roles, type, shape, elevation and motion every component draws from.",
    "page": "category-foundations",
    "count": 5
  },
  {
    "id": "actions",
    "title": "Actions",
    "description": "Buttons that start something, or turn a setting on and off.",
    "page": "category-actions",
    "count": 4
  },
  {
    "id": "inputs",
    "title": "Inputs",
    "description": "Fields a person types into or sets: text, dates, times and amounts.",
    "page": "category-inputs",
    "count": 8
  },
  {
    "id": "selection",
    "title": "Selection",
    "description": "Controls for choosing among options: one, several, or on and off.",
    "page": "category-selection",
    "count": 7
  },
  {
    "id": "navigation",
    "title": "Navigation",
    "description": "Ways to move between views on the same page.",
    "page": "category-navigation",
    "count": 2
  },
  {
    "id": "feedback",
    "title": "Feedback",
    "description": "Status, progress and confirmation, from a badge to a toast.",
    "page": "category-feedback",
    "count": 6
  },
  {
    "id": "overlays",
    "title": "Overlays",
    "description": "Surfaces that float over the page: dialogs, sheets, menus and popovers.",
    "page": "category-overlays",
    "count": 5
  },
  {
    "id": "content",
    "title": "Content",
    "description": "Containers and rows that hold what the app shows.",
    "page": "category-content",
    "count": 8
  },
  {
    "id": "charts",
    "title": "Charts",
    "description": "Figures and small charts that show numbers at a glance.",
    "page": "category-charts",
    "count": 6
  },
  {
    "id": "media",
    "title": "Media",
    "description": "Video and its controls.",
    "page": "category-media",
    "count": 2
  },
  {
    "id": "patterns",
    "title": "Patterns",
    "description": "Whole screens built from the components: forms, settings and long-running work.",
    "page": "category-patterns",
    "count": 3
  }
];

/** The 6 latest components, newest first. */
export const newest: string[] = ["CheckboxList","Combobox","Markdown","NumberInput","SearchField","Table"];

export const version = "0.1.0";
export const packageStatus = "Preview";
