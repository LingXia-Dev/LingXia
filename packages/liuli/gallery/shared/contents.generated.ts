// Generated from the doc file by scripts/generate.mjs; do not edit.
import type { Component } from '@lingxia/view';
import { Content as CategoryFoundations } from '../content/category-foundations';
import { Content as CategoryActions } from '../content/category-actions';
import { Content as CategoryInputs } from '../content/category-inputs';
import { Content as CategorySelection } from '../content/category-selection';
import { Content as CategoryNavigation } from '../content/category-navigation';
import { Content as CategoryFeedback } from '../content/category-feedback';
import { Content as CategoryOverlays } from '../content/category-overlays';
import { Content as CategoryContent } from '../content/category-content';
import { Content as CategoryCharts } from '../content/category-charts';
import { Content as CategoryMedia } from '../content/category-media';
import { Content as CategoryPatterns } from '../content/category-patterns';
import { Content as Colour } from '../content/colour';
import { Content as Typography } from '../content/typography';
import { Content as ShapeAndSpacing } from '../content/shape-and-spacing';
import { Content as Elevation } from '../content/elevation';
import { Content as Motion } from '../content/motion';
import { Content as Button } from '../content/button';
import { Content as ToggleButton } from '../content/toggle-button';
import { Content as IconButton } from '../content/icon-button';
import { Content as ButtonGroup } from '../content/button-group';
import { Content as TextInput } from '../content/text-input';
import { Content as TextArea } from '../content/text-area';
import { Content as DateInput } from '../content/date-input';
import { Content as TimeInput } from '../content/time-input';
import { Content as Slider } from '../content/slider';
import { Content as Field } from '../content/field';
import { Content as SearchField } from '../content/search-field';
import { Content as NumberInput } from '../content/number-input';
import { Content as Calendar } from '../content/calendar';
import { Content as DateRangeInput } from '../content/date-range-input';
import { Content as FormLayout } from '../content/form-layout';
import { Content as DateTimeInput } from '../content/date-time-input';
import { Content as FileInput } from '../content/file-input';
import { Content as Composer } from '../content/composer';
import { Content as CheckboxInput } from '../content/checkbox-input';
import { Content as RadioGroup } from '../content/radio-group';
import { Content as Switch } from '../content/switch';
import { Content as Select } from '../content/select';
import { Content as SegmentedControl } from '../content/segmented-control';
import { Content as Combobox } from '../content/combobox';
import { Content as CheckboxList } from '../content/checkbox-list';
import { Content as Token } from '../content/token';
import { Content as MultiSelect } from '../content/multi-select';
import { Content as Tabs } from '../content/tabs';
import { Content as NavList } from '../content/nav-list';
import { Content as Breadcrumb } from '../content/breadcrumb';
import { Content as Pagination } from '../content/pagination';
import { Content as Stepper } from '../content/stepper';
import { Content as Link } from '../content/link';
import { Content as PageControl } from '../content/page-control';
import { Content as TreeList } from '../content/tree-list';
import { Content as Badge } from '../content/badge';
import { Content as ProgressBar } from '../content/progress-bar';
import { Content as Banner } from '../content/banner';
import { Content as EmptyState } from '../content/empty-state';
import { Content as Skeleton } from '../content/skeleton';
import { Content as Toast } from '../content/toast';
import { Content as CircularProgress } from '../content/circular-progress';
import { Content as StatusDot } from '../content/status-dot';
import { Content as Dialog } from '../content/dialog';
import { Content as DropdownMenu } from '../content/dropdown-menu';
import { Content as Tooltip } from '../content/tooltip';
import { Content as BottomSheet } from '../content/bottom-sheet';
import { Content as Popover } from '../content/popover';
import { Content as Drawer } from '../content/drawer';
import { Content as HoverCard } from '../content/hover-card';
import { Content as ContextMenu } from '../content/context-menu';
import { Content as CommandPalette } from '../content/command-palette';
import { Content as Tour } from '../content/tour';
import { Content as Card } from '../content/card';
import { Content as Item } from '../content/item';
import { Content as Details } from '../content/details';
import { Content as Avatar } from '../content/avatar';
import { Content as SplitView } from '../content/split-view';
import { Content as Icon } from '../content/icon';
import { Content as Table } from '../content/table';
import { Content as Markdown } from '../content/markdown';
import { Content as MetadataList } from '../content/metadata-list';
import { Content as Section } from '../content/section';
import { Content as Divider } from '../content/divider';
import { Content as Kbd } from '../content/kbd';
import { Content as Timer } from '../content/timer';
import { Content as Code } from '../content/code';
import { Content as Timestamp } from '../content/timestamp';
import { Content as MessageBubble } from '../content/message-bubble';
import { Content as Stat } from '../content/stat';
import { Content as Sparkline } from '../content/sparkline';
import { Content as BarChart } from '../content/bar-chart';
import { Content as LineChart } from '../content/line-chart';
import { Content as PieChart } from '../content/pie-chart';
import { Content as Chart } from '../content/chart';
import { Content as VideoPlayer } from '../content/video-player';
import { Content as VideoControls } from '../content/video-controls';
import { Content as Carousel } from '../content/carousel';
import { Content as Thumbnail } from '../content/thumbnail';
import { Content as Lightbox } from '../content/lightbox';
import { Content as SignIn } from '../content/sign-in';
import { Content as Settings } from '../content/settings';
import { Content as Uploads } from '../content/uploads';
import { Content as ListDetail } from '../content/list-detail';
import { Content as Dashboard } from '../content/dashboard';

/** Category contents open a page through `onOpen`; the others ignore it. */
export type GalleryContent = Component<{ onOpen: (page: string) => void }>;

export const contents: Record<string, GalleryContent> = {
  "category-foundations": CategoryFoundations,
  "category-actions": CategoryActions,
  "category-inputs": CategoryInputs,
  "category-selection": CategorySelection,
  "category-navigation": CategoryNavigation,
  "category-feedback": CategoryFeedback,
  "category-overlays": CategoryOverlays,
  "category-content": CategoryContent,
  "category-charts": CategoryCharts,
  "category-media": CategoryMedia,
  "category-patterns": CategoryPatterns,
  "colour": Colour,
  "typography": Typography,
  "shape-and-spacing": ShapeAndSpacing,
  "elevation": Elevation,
  "motion": Motion,
  "button": Button,
  "toggle-button": ToggleButton,
  "icon-button": IconButton,
  "button-group": ButtonGroup,
  "text-input": TextInput,
  "text-area": TextArea,
  "date-input": DateInput,
  "time-input": TimeInput,
  "slider": Slider,
  "field": Field,
  "search-field": SearchField,
  "number-input": NumberInput,
  "calendar": Calendar,
  "date-range-input": DateRangeInput,
  "form-layout": FormLayout,
  "date-time-input": DateTimeInput,
  "file-input": FileInput,
  "composer": Composer,
  "checkbox-input": CheckboxInput,
  "radio-group": RadioGroup,
  "switch": Switch,
  "select": Select,
  "segmented-control": SegmentedControl,
  "combobox": Combobox,
  "checkbox-list": CheckboxList,
  "token": Token,
  "multi-select": MultiSelect,
  "tabs": Tabs,
  "nav-list": NavList,
  "breadcrumb": Breadcrumb,
  "pagination": Pagination,
  "stepper": Stepper,
  "link": Link,
  "page-control": PageControl,
  "tree-list": TreeList,
  "badge": Badge,
  "progress-bar": ProgressBar,
  "banner": Banner,
  "empty-state": EmptyState,
  "skeleton": Skeleton,
  "toast": Toast,
  "circular-progress": CircularProgress,
  "status-dot": StatusDot,
  "dialog": Dialog,
  "dropdown-menu": DropdownMenu,
  "tooltip": Tooltip,
  "bottom-sheet": BottomSheet,
  "popover": Popover,
  "drawer": Drawer,
  "hover-card": HoverCard,
  "context-menu": ContextMenu,
  "command-palette": CommandPalette,
  "tour": Tour,
  "card": Card,
  "item": Item,
  "details": Details,
  "avatar": Avatar,
  "split-view": SplitView,
  "icon": Icon,
  "table": Table,
  "markdown": Markdown,
  "metadata-list": MetadataList,
  "section": Section,
  "divider": Divider,
  "kbd": Kbd,
  "timer": Timer,
  "code": Code,
  "timestamp": Timestamp,
  "message-bubble": MessageBubble,
  "stat": Stat,
  "sparkline": Sparkline,
  "bar-chart": BarChart,
  "line-chart": LineChart,
  "pie-chart": PieChart,
  "chart": Chart,
  "video-player": VideoPlayer,
  "video-controls": VideoControls,
  "carousel": Carousel,
  "thumbnail": Thumbnail,
  "lightbox": Lightbox,
  "sign-in": SignIn,
  "settings": Settings,
  "uploads": Uploads,
  "list-detail": ListDetail,
  "dashboard": Dashboard,
};
