// Each line is one authoring mistake; authoring-types.test.mjs checks tsc names its own prop.
import { useLocal } from '../../src/index.js';

export const open = <view open/>;
export const manualClose = <view popover="manual" onCloseRequest={() => false}/>;
export const undragged = <view aria-label="Sheet" dragSnapPoints={[0, 100]}/>;
export const deaf = <scroll onSnapChange={() => {}}/>;
export const deafDrag = <view drag="y" aria-label="Sheet" onSnapChange={() => {}}/>;
export function Both() { const cell = useLocal(0); return <scroll bind={cell} snapIndex={0}/>; }
export const actions = <button aria-actions="close" onClick={() => {}}>Close</button>;
export const openAndSnap = <view open aria-label="Sheet" dragSnapPoints={[0, 100]}/>;
export const closeAndSnap = <view popover onCloseRequest={() => false} dragSnapPoints={[0]}/>;
export const unheard = <scroll snapIndex={0}/>;
export const silent = <video aria-label="Clip"/>;
export const twice = <video src="a.mp4" sources={[{ src: 'b.mp4', label: '720p' }]}/>;
