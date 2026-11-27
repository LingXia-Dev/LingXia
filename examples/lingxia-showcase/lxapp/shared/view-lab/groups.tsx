// Every group of the View catalogue with the bodies of its demos: one source for a group's own
// page and for the index page showing a group in place.
import { GroupPage } from './demo';
import { accessibilityDemos } from './accessibility';
import { appearanceDemos } from './appearance';
import { controlDemos, drawingDemos, mediaDemos, webDemos } from './elements';
import { gestureDemos, pointerDemos } from './gestures';
import { layoutDemos, pinnedRoom } from './layout';
import { motionDemos } from './motion';
import { overlayDemos } from './overlays';
import { scrollingDemos } from './scrolling';
import { inputDemos, textDemos } from './text-input';
import type { GroupId } from './text';
import type { View } from '../../pages/view/$types';

/** One group's page, chosen by id. Only the chosen group renders. */
export function Group({ id, view }: { id: GroupId; view: View }) {
  switch (id) {
    case 'drawing': return <GroupPage id="drawing" view={view} bodies={drawingDemos} />;
    case 'media': return <GroupPage id="media" view={view} bodies={mediaDemos} />;
    case 'web': return <GroupPage id="web" view={view} bodies={webDemos} />;
    case 'controls': return <GroupPage id="controls" view={view} bodies={controlDemos} />;
    case 'layout': return <GroupPage id="layout" view={view} bodies={layoutDemos} endRoom={pinnedRoom} />;
    case 'text': return <GroupPage id="text" view={view} bodies={textDemos} />;
    case 'input': return <GroupPage id="input" view={view} bodies={inputDemos} />;
    case 'gestures': return <GroupPage id="gestures" view={view} bodies={gestureDemos} />;
    case 'pointer': return <GroupPage id="pointer" view={view} bodies={pointerDemos} />;
    case 'overlays': return <GroupPage id="overlays" view={view} bodies={overlayDemos} />;
    case 'scrolling': return <GroupPage id="scrolling" view={view} bodies={scrollingDemos} />;
    case 'motion': return <GroupPage id="motion" view={view} bodies={motionDemos} />;
    case 'appearance': return <GroupPage id="appearance" view={view} bodies={appearanceDemos} />;
    case 'accessibility': return <GroupPage id="accessibility" view={view} bodies={accessibilityDemos} />;
  }
}
