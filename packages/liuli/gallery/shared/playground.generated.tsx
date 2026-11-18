// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Avatar } from '@liulidesign/core/Avatar';
import { Badge } from '@liulidesign/core/Badge';
import { Banner } from '@liulidesign/core/Banner';
import { Button } from '@liulidesign/core/Button';
import { CircularProgress } from '@liulidesign/core/CircularProgress';
import { EmptyState } from '@liulidesign/core/EmptyState';
import { ProgressBar } from '@liulidesign/core/ProgressBar';
import { StatusDot } from '@liulidesign/core/StatusDot';
import { Timer } from '@liulidesign/core/Timer';
import { Token } from '@liulidesign/core/Token';

export interface PlaygroundControl { name: string; required: boolean; kind: 'boolean' | 'text' | 'number' | 'choice'; options?: string[] }
export type PlaygroundProps = Record<string, string | number | boolean>;
export interface PlaygroundEntry { name: string; defaults: PlaygroundProps; controls: PlaygroundControl[] }

export const playgroundEntries: PlaygroundEntry[] = [
  {
    "name": "Avatar",
    "defaults": {
      "name": "Ana Lima",
      "size": "md",
      "shape": "circle",
      "status": "online",
      "statusLabel": "Online"
    },
    "controls": [
      {
        "name": "name",
        "required": true,
        "kind": "text"
      },
      {
        "name": "src",
        "required": false,
        "kind": "text"
      },
      {
        "name": "initials",
        "required": false,
        "kind": "text"
      },
      {
        "name": "size",
        "required": false,
        "kind": "choice",
        "options": [
          "xs",
          "sm",
          "md",
          "lg"
        ]
      },
      {
        "name": "shape",
        "required": false,
        "kind": "choice",
        "options": [
          "circle",
          "rounded"
        ]
      },
      {
        "name": "status",
        "required": false,
        "kind": "choice",
        "options": [
          "away",
          "busy",
          "offline",
          "online"
        ]
      },
      {
        "name": "statusLabel",
        "required": false,
        "kind": "text"
      },
      {
        "name": "surface",
        "required": false,
        "kind": "choice",
        "options": [
          "raised",
          "sunken",
          "surface"
        ]
      },
      {
        "name": "isDecorative",
        "required": false,
        "kind": "boolean"
      }
    ]
  },
  {
    "name": "Badge",
    "defaults": {
      "label": "In review",
      "tone": "warning",
      "size": "md"
    },
    "controls": [
      {
        "name": "tone",
        "required": false,
        "kind": "choice",
        "options": [
          "accent",
          "danger",
          "info",
          "neutral",
          "success",
          "warning"
        ]
      },
      {
        "name": "emphasis",
        "required": false,
        "kind": "choice",
        "options": [
          "soft",
          "solid"
        ]
      },
      {
        "name": "label",
        "required": true,
        "kind": "text"
      },
      {
        "name": "size",
        "required": false,
        "kind": "choice",
        "options": [
          "sm",
          "md"
        ]
      }
    ]
  },
  {
    "name": "Banner",
    "defaults": {
      "title": "Storage almost full",
      "message": "You have used 9.6 of 10 GB.",
      "tone": "warning"
    },
    "controls": [
      {
        "name": "dismissLabel",
        "required": false,
        "kind": "text"
      },
      {
        "name": "isCollapsible",
        "required": false,
        "kind": "boolean"
      },
      {
        "name": "toggleLabel",
        "required": false,
        "kind": "text"
      },
      {
        "name": "isDefaultOpen",
        "required": false,
        "kind": "boolean"
      },
      {
        "name": "title",
        "required": true,
        "kind": "text"
      },
      {
        "name": "message",
        "required": false,
        "kind": "text"
      },
      {
        "name": "tone",
        "required": false,
        "kind": "choice",
        "options": [
          "danger",
          "info",
          "neutral",
          "success",
          "warning"
        ]
      }
    ]
  },
  {
    "name": "Button",
    "defaults": {
      "label": "Save changes",
      "variant": "primary",
      "size": "md",
      "isDisabled": false,
      "isLoading": false,
      "isFloating": false
    },
    "controls": [
      {
        "name": "label",
        "required": true,
        "kind": "text"
      },
      {
        "name": "variant",
        "required": false,
        "kind": "choice",
        "options": [
          "destructive",
          "ghost",
          "link",
          "primary",
          "secondary"
        ]
      },
      {
        "name": "size",
        "required": false,
        "kind": "choice",
        "options": [
          "sm",
          "md",
          "lg"
        ]
      },
      {
        "name": "isIconOnly",
        "required": false,
        "kind": "boolean"
      },
      {
        "name": "isFloating",
        "required": false,
        "kind": "boolean"
      },
      {
        "name": "width",
        "required": false,
        "kind": "choice",
        "options": [
          "content",
          "fill"
        ]
      },
      {
        "name": "isDisabled",
        "required": false,
        "kind": "boolean"
      },
      {
        "name": "disabledMessage",
        "required": false,
        "kind": "text"
      },
      {
        "name": "isLoading",
        "required": false,
        "kind": "boolean"
      }
    ]
  },
  {
    "name": "CircularProgress",
    "defaults": {
      "label": "Steps today",
      "value": 72,
      "valueLabel": "72%",
      "size": "lg",
      "tone": "accent"
    },
    "controls": [
      {
        "name": "label",
        "required": true,
        "kind": "text"
      },
      {
        "name": "value",
        "required": false,
        "kind": "number"
      },
      {
        "name": "max",
        "required": false,
        "kind": "number"
      },
      {
        "name": "valueLabel",
        "required": false,
        "kind": "text"
      },
      {
        "name": "size",
        "required": false,
        "kind": "choice",
        "options": [
          "sm",
          "md",
          "lg"
        ]
      },
      {
        "name": "tone",
        "required": false,
        "kind": "choice",
        "options": [
          "accent",
          "danger",
          "success",
          "warning"
        ]
      }
    ]
  },
  {
    "name": "EmptyState",
    "defaults": {
      "title": "No notes yet",
      "description": "Notes you write show up here.",
      "size": "md"
    },
    "controls": [
      {
        "name": "title",
        "required": true,
        "kind": "text"
      },
      {
        "name": "description",
        "required": false,
        "kind": "text"
      },
      {
        "name": "size",
        "required": false,
        "kind": "choice",
        "options": [
          "sm",
          "md"
        ]
      }
    ]
  },
  {
    "name": "ProgressBar",
    "defaults": {
      "label": "Uploading photos",
      "value": 40,
      "valueLabel": "3 of 8",
      "size": "md",
      "tone": "accent"
    },
    "controls": [
      {
        "name": "label",
        "required": true,
        "kind": "text"
      },
      {
        "name": "isLabelHidden",
        "required": false,
        "kind": "boolean"
      },
      {
        "name": "value",
        "required": false,
        "kind": "number"
      },
      {
        "name": "max",
        "required": false,
        "kind": "number"
      },
      {
        "name": "valueLabel",
        "required": false,
        "kind": "text"
      },
      {
        "name": "size",
        "required": false,
        "kind": "choice",
        "options": [
          "sm",
          "md"
        ]
      },
      {
        "name": "tone",
        "required": false,
        "kind": "choice",
        "options": [
          "accent",
          "danger",
          "success",
          "warning"
        ]
      }
    ]
  },
  {
    "name": "StatusDot",
    "defaults": {
      "label": "Running",
      "tone": "success",
      "size": "md",
      "isLabelHidden": false
    },
    "controls": [
      {
        "name": "label",
        "required": true,
        "kind": "text"
      },
      {
        "name": "isLabelHidden",
        "required": false,
        "kind": "boolean"
      },
      {
        "name": "tone",
        "required": false,
        "kind": "choice",
        "options": [
          "accent",
          "danger",
          "info",
          "neutral",
          "success",
          "warning"
        ]
      },
      {
        "name": "size",
        "required": false,
        "kind": "choice",
        "options": [
          "sm",
          "md"
        ]
      }
    ]
  },
  {
    "name": "Timer",
    "defaults": {
      "seconds": 125,
      "size": "lg",
      "hasHours": false
    },
    "controls": [
      {
        "name": "seconds",
        "required": true,
        "kind": "number"
      },
      {
        "name": "hasHours",
        "required": false,
        "kind": "boolean"
      },
      {
        "name": "size",
        "required": false,
        "kind": "choice",
        "options": [
          "sm",
          "md",
          "lg"
        ]
      }
    ]
  },
  {
    "name": "Token",
    "defaults": {
      "label": "Travel",
      "size": "md",
      "isSelected": false,
      "isDisabled": false
    },
    "controls": [
      {
        "name": "removeLabel",
        "required": false,
        "kind": "text"
      },
      {
        "name": "label",
        "required": true,
        "kind": "text"
      },
      {
        "name": "isSelected",
        "required": false,
        "kind": "boolean"
      },
      {
        "name": "size",
        "required": false,
        "kind": "choice",
        "options": [
          "sm",
          "md"
        ]
      },
      {
        "name": "isDisabled",
        "required": false,
        "kind": "boolean"
      }
    ]
  }
];

/** The chosen component, drawn from the props in hand. */
export function PlaygroundSubject({ name, props }: { name: string; props: PlaygroundProps }) {
  if (name === "Avatar") return <Avatar {...(props as any)} />;
  if (name === "Badge") return <Badge {...(props as any)} />;
  if (name === "Banner") return <Banner {...(props as any)} />;
  if (name === "Button") return <Button {...(props as any)} />;
  if (name === "CircularProgress") return <CircularProgress {...(props as any)} />;
  if (name === "EmptyState") return <EmptyState {...(props as any)} />;
  if (name === "ProgressBar") return <ProgressBar {...(props as any)} />;
  if (name === "StatusDot") return <StatusDot {...(props as any)} />;
  if (name === "Timer") return <Timer {...(props as any)} />;
  if (name === "Token") return <Token {...(props as any)} />;
  return null;
}
