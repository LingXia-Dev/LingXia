// Generated from the doc file by scripts/generate.mjs; do not edit.

import type { IconName } from '@liulidesign/core/Icon';

export interface GalleryEntry { name: string; page: string; category: string; summary: string; keywords: string[] }
export interface GalleryCategory { id: string; title: string; icon: IconName; description: string; page: string; count: number }

export const entries: GalleryEntry[] = [
  {
    "name": "Colour",
    "page": "colour",
    "category": "foundations",
    "summary": "The theme's colour roles and what each is for",
    "keywords": []
  },
  {
    "name": "Typography",
    "page": "typography",
    "category": "foundations",
    "summary": "Six type sizes, their line heights and weights",
    "keywords": []
  },
  {
    "name": "Shape & spacing",
    "page": "shape-and-spacing",
    "category": "foundations",
    "summary": "Corner radii, control heights, spacing steps and focus shapes",
    "keywords": []
  },
  {
    "name": "Elevation",
    "page": "elevation",
    "category": "foundations",
    "summary": "Flat, resting, raised and modal surfaces",
    "keywords": []
  },
  {
    "name": "Motion",
    "page": "motion",
    "category": "foundations",
    "summary": "How things enter, move and respond, and reduced motion",
    "keywords": []
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
  },
  {
    "name": "Calendar",
    "page": "calendar",
    "category": "inputs",
    "summary": "A month of days in the page, to choose one day or a span",
    "keywords": [
      "calendar",
      "date picker",
      "date range",
      "month",
      "day picker",
      "range picker",
      "schedule"
    ]
  },
  {
    "name": "DateRangeInput",
    "page": "date-range-input",
    "category": "inputs",
    "summary": "A span of days in a form: a field that opens a calendar",
    "keywords": [
      "date range",
      "date range picker",
      "period",
      "stay",
      "from to",
      "range input",
      "calendar field"
    ]
  },
  {
    "name": "FormLayout",
    "page": "form-layout",
    "category": "inputs",
    "summary": "A form's fields at one rhythm: a column on a phone, two where there is room",
    "keywords": [
      "form",
      "form layout",
      "fields",
      "two column form",
      "form grid",
      "form actions"
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
  },
  {
    "name": "Breadcrumb",
    "page": "breadcrumb",
    "category": "navigation",
    "summary": "Where a page sits: the levels above it, each a way back up",
    "keywords": [
      "breadcrumb",
      "breadcrumbs",
      "path",
      "trail",
      "hierarchy",
      "location",
      "back"
    ]
  },
  {
    "name": "Pagination",
    "page": "pagination",
    "category": "navigation",
    "summary": "Moves through a long set a page at a time",
    "keywords": [
      "pagination",
      "pager",
      "pages",
      "page numbers",
      "next page",
      "previous page",
      "paging"
    ]
  },
  {
    "name": "Stepper",
    "page": "stepper",
    "category": "navigation",
    "summary": "Where someone is in a task with a known number of steps",
    "keywords": [
      "stepper",
      "steps",
      "wizard",
      "progress steps",
      "multi-step",
      "checkout",
      "onboarding"
    ]
  },
  {
    "name": "Link",
    "page": "link",
    "category": "navigation",
    "summary": "A link among the words of a sentence",
    "keywords": [
      "link",
      "anchor",
      "hyperlink",
      "inline",
      "text"
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
  },
  {
    "name": "MetadataList",
    "page": "metadata-list",
    "category": "content",
    "summary": "What is known about one thing, as labels and their values",
    "keywords": [
      "metadata",
      "key value",
      "details",
      "description list",
      "properties",
      "facts",
      "definition list"
    ]
  },
  {
    "name": "Section",
    "page": "section",
    "category": "content",
    "summary": "A titled part of a page: a heading, then its content",
    "keywords": [
      "section",
      "heading",
      "group",
      "page section",
      "header",
      "titled block"
    ]
  },
  {
    "name": "Divider",
    "page": "divider",
    "category": "content",
    "summary": "A hairline between two things that belong apart",
    "keywords": [
      "divider",
      "separator",
      "rule",
      "line",
      "hairline",
      "hr"
    ]
  },
  {
    "name": "Kbd",
    "page": "kbd",
    "category": "content",
    "summary": "A keyboard shortcut, drawn as key caps",
    "keywords": [
      "kbd",
      "keyboard",
      "shortcut",
      "key",
      "hotkey",
      "accelerator"
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
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
    ]
  },
  {
    "name": "Sign in",
    "page": "sign-in",
    "category": "patterns",
    "summary": "A sign-in form that validates and reports errors",
    "keywords": []
  },
  {
    "name": "Settings",
    "page": "settings",
    "category": "patterns",
    "summary": "A settings screen of rows, switches and a confirmation",
    "keywords": []
  },
  {
    "name": "Uploads",
    "page": "uploads",
    "category": "patterns",
    "summary": "Long-running work with progress, failure and retry",
    "keywords": []
  },
  {
    "name": "List & detail",
    "page": "list-detail",
    "category": "patterns",
    "summary": "A list of records beside the open one: two panes on a wide screen, one at a time on a phone",
    "keywords": []
  },
  {
    "name": "Dashboard",
    "page": "dashboard",
    "category": "patterns",
    "summary": "A period's figures, their charts and a table, laid out for a phone and for a desk",
    "keywords": []
  }
];

export const galleryCategories: GalleryCategory[] = [
  {
    "id": "foundations",
    "title": "Foundations",
    "icon": "palette",
    "description": "Colour, type, shape and motion",
    "page": "category-foundations",
    "count": 5
  },
  {
    "id": "actions",
    "title": "Actions",
    "icon": "click",
    "description": "Buttons and toggles",
    "page": "category-actions",
    "count": 4
  },
  {
    "id": "inputs",
    "title": "Inputs",
    "icon": "text-input",
    "description": "Text, dates and amounts",
    "page": "category-inputs",
    "count": 11
  },
  {
    "id": "selection",
    "title": "Selection",
    "icon": "checklist",
    "description": "One, several, on or off",
    "page": "category-selection",
    "count": 7
  },
  {
    "id": "navigation",
    "title": "Navigation",
    "icon": "compass",
    "description": "Tabs, steps and pages",
    "page": "category-navigation",
    "count": 6
  },
  {
    "id": "feedback",
    "title": "Feedback",
    "icon": "notifications",
    "description": "Status and progress",
    "page": "category-feedback",
    "count": 6
  },
  {
    "id": "overlays",
    "title": "Overlays",
    "icon": "layers",
    "description": "Dialogs, sheets and menus",
    "page": "category-overlays",
    "count": 5
  },
  {
    "id": "content",
    "title": "Content",
    "icon": "rows",
    "description": "Cards, rows and tables",
    "page": "category-content",
    "count": 12
  },
  {
    "id": "charts",
    "title": "Charts",
    "icon": "chart",
    "description": "Figures and small charts",
    "page": "category-charts",
    "count": 6
  },
  {
    "id": "media",
    "title": "Media",
    "icon": "play",
    "description": "Video and its controls",
    "page": "category-media",
    "count": 2
  },
  {
    "id": "patterns",
    "title": "Patterns",
    "icon": "template",
    "description": "Whole screens, put together",
    "page": "category-patterns",
    "count": 5
  }
];

export const version = "0.1.0";
export const packageStatus = "Preview";
