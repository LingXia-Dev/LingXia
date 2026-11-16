// Every group of the View catalogue with the bodies of its demos: one source for a group's own
// page and for the index page showing a group in place.
import { GroupPage } from './demo';
import { accessibilityDemos } from './accessibility';
import { appearanceDemos } from './appearance';
import { controlDemos, drawingDemos, mediaDemos } from './elements';
import { gestureDemos, pointerDemos } from './gestures';
import { layoutDemos } from './layout';
import { motionDemos } from './motion';
import { overlayDemos } from './overlays';
import { scrollingDemos } from './scrolling';
import { inputDemos, textDemos } from './text-input';
import type { GroupId } from './text';

/** One group's page, chosen by id. Only the chosen group renders. */
export function Group({ id }: { id: GroupId }) {
  switch (id) {
    case 'drawing': return <GroupPage id="drawing" bodies={drawingDemos} />;
    case 'media': return <GroupPage id="media" bodies={mediaDemos} />;
    case 'controls': return <GroupPage id="controls" bodies={controlDemos} />;
    case 'layout': return <GroupPage id="layout" bodies={layoutDemos} />;
    case 'text': return <GroupPage id="text" bodies={textDemos} />;
    case 'input': return <GroupPage id="input" bodies={inputDemos} />;
    case 'gestures': return <GroupPage id="gestures" bodies={gestureDemos} />;
    case 'pointer': return <GroupPage id="pointer" bodies={pointerDemos} />;
    case 'overlays': return <GroupPage id="overlays" bodies={overlayDemos} />;
    case 'scrolling': return <GroupPage id="scrolling" bodies={scrollingDemos} />;
    case 'motion': return <GroupPage id="motion" bodies={motionDemos} />;
    case 'appearance': return <GroupPage id="appearance" bodies={appearanceDemos} />;
    case 'accessibility': return <GroupPage id="accessibility" bodies={accessibilityDemos} />;
  }
}
