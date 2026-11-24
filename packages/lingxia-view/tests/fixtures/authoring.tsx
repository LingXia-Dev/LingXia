import { cx, List, useLocal, useRequest, useStream, useChannel, useLeaveGuard, useView, type Channel, type ChannelEndpoint, type Stream, type Props, type ViewProps, type VectorProps, type ButtonProps, type View, type Local } from '../../src/index.js';

// A role is typed per element, so a component forwarding to a <view> takes ViewProps.
function Card(props: ViewProps) { return <view {...props}/>; }
// @ts-expect-error Props carries every role, and a <view> takes only its own.
function Loose(props: Props) { return <view {...props}/>; }
// A wrapper that fixes some props forwards the rest, whatever companion rules they carry.
function Icon({ name, ...props }: Omit<VectorProps, 'paths'> & { name: string }) { return <vector {...props} paths={[{ d: name }]}/>; }
function Shell({ className, ...props }: ViewProps) { return <view {...props} className={cx('p-2', className)}/>; }
function Press({ className, ...props }: ButtonProps) { return <button {...props} className={cx('p-2', className)}/>; }
export function render(view: View<{ save(value: string): Promise<void> }, { title: string; rows: { text: string }[]; pair: [string, number] }>) {
  const draft = useLocal('draft', '');
  const checked = useLocal('checked', false);
  const volume = useLocal('volume', 1);
  const nested = useLocal('nested', {items: [{title: 'one'}], pair: ['one', 1] as [string, number]});
  nested.set(nested.value);
  nested.set({...nested.value, items: [...nested.value.items, {title: 'two'}]});
  volume.set(level => level + 0.1);
  nested.set(latest => ({...latest, items: [...latest.items, {title: 'three'}]}));
  // @ts-expect-error an updater returns the cell's type
  volume.set(level => String(level));
  // @ts-expect-error the latest value an updater gets is immutable
  nested.set(latest => { latest.items.push({title: 'four'}); return latest; });
  // @ts-expect-error local snapshots are recursively immutable
  nested.value.items.push({title: 'two'});
  // @ts-expect-error nested local records are immutable
  nested.value.items[0].title = 'two';
  // @ts-expect-error local tuples preserve readonly positions
  nested.value.pair[1] = 2;
  view.actions.save(view.data.title);
  // @ts-expect-error data is the page contract, not an untyped dictionary
  view.data.missing;
  // @ts-expect-error page data is immutable
  view.data.title = 'replacement';
  // @ts-expect-error nested records are frozen by the runtime too
  view.data.rows[0].text = 'replacement';
  // @ts-expect-error replicated arrays cannot be mutated in View code
  view.data.rows.push({ text: 'new' });
  // @ts-expect-error tuples retain both their positions and readonly contract
  view.data.pair[1] = 2;
  const pairValue: number = view.data.pair[1];
  // @ts-expect-error action payload type comes from the contract
  view.actions.save(3);
  // @ts-expect-error bound editors cannot also be controlled
  const input = <input bind={draft} value="conflict"/>;
  // @ts-expect-error textarea preserves the binding union
  const textarea = <textarea bind={draft} onInput={() => {}}/>;
  // @ts-expect-error Omit must not erase the search binding union
  const search = <input type="search" bind={draft} value="conflict"/>;
  // @ts-expect-error style properties are checked per element
  const misplaced = <text style={{alignItems: 'center'}}/>;
  // @ts-expect-error grid columns are a style, not a prop
  const columns = <view className="grid" columns={2}/>;
  const grid = <view className="grid" style={{gridTemplateColumns: 2}}><text style={{lineClamp: 2}}>Card</text></view>;
  const disabled = <button disabled onClick={() => {}}>Save</button>;
  // @ts-expect-error controls are disabled, not enabled
  const enabled = <button enabled={false}/>;
  const composed = <button onClick={() => {}}><vector paths={[{d: 'M0 0'}]}/><text>Inbox</text></button>;
  const named = <switch bind={checked} aria-label="Sync"/>;
  // @ts-expect-error boolean binding has one owner
  const toggle = <switch bind={checked} onChange={() => {}}/>;
  // @ts-expect-error checkbox preserves the binding union
  const checkbox = <checkbox bind={checked} value={false}/>;
  // @ts-expect-error numeric binding has one owner
  const slider = <slider bind={volume} value={1} aria-label="Volume"/>;
  // @ts-expect-error radio binding has one owner
  const radio = <radio bind={draft} value="a" options={[]}/>;
  // @ts-expect-error objects are not renderable children
  const child = <Card>{{ arbitrary: true }}</Card>;
  // @ts-expect-error native text accepts text and runs, not objects
  const nestedText = <text>{{ arbitrary: true }}</text>;
  const rich = <text>Read <span className="font-bold">this</span> or <link onClick={() => {}}>that</link></text>;
  // @ts-expect-error editors are leaves
  const inputChildren = <input><text>invalid</text></input>;
  // @ts-expect-error the list element is reached only through <List>
  const rawList = <list count={0} row={() => <text/>}/>;
  const layout = <view className="flex flex-col md:flex-row" style={{flexDirection:'row-reverse'}}/>;
  const meaningfulImage = <image src="photo.png" alt="A mountain lake"/>;
  const recoverableImage = <image src="photo.png" retryKey="1" onLoad={()=>{}} onError={()=>{}}/>;
  const namedInput = <input aria-label="Name" bind={draft}/>;
  const decorativeImage = <image src="texture.png" alt=""/>;
  // @ts-expect-error alternative text must be text
  const invalidAlt = <image src="photo.png" alt={3}/>;
  const sheet = <view popover="manual" role="dialog" aria-modal aria-label="Sheet"/>;
  // @ts-expect-error roles are a closed set.
  const banner = <view role="banner"/>;
  const composite = [<button role="tab" onClick={() => {}}>A</button>, <view role="tablist"/>, <input role="combobox" aria-label="City"/>,
    <dialog role="alertdialog" aria-label="Delete"/>, <text role="img" aria-label="4 of 5">★★★★☆</text>];
  // @ts-expect-error role="tab" belongs on a <button>.
  const tabOnView = <view role="tab"/>;
  // @ts-expect-error role="tablist" belongs on a <view>.
  const tablistOnButton = <button role="tablist" onClick={() => {}}>x</button>;
  // @ts-expect-error role="spinbutton" belongs on an <input>.
  const spinOnText = <text role="spinbutton">1</text>;
  const run = <text>A <span className="font-bold" data-testid="b">b</span></text>;
  // @ts-expect-error a <span> is a run of its <text>; the id goes on the <text>.
  const runId = <text><span id="x">b</span></text>;
  // @ts-expect-error a run carries no role of its own.
  const runRole = <text><span role="heading">b</span></text>;
  const menu = <view id="m" popover open onToggle={(open: boolean) => {}} anchor="b" onCloseRequest={() => false}/>;
  const note = <view id="n" popover="manual" open closeAfter={3000}/>;
  // @ts-expect-error open and onToggle need popover.
  const unpopped = <view open onToggle={(open: boolean) => {}}/>;
  // @ts-expect-error nothing but your code closes a manual popover, so it hears no close request.
  const manualClose = <view popover="manual" onCloseRequest={() => false}/>;
  const sheet2 = <view drag="y" dragSnapPoints={[0, '50%']} aria-label="Sheet" aria-actions="close" onDragEnd={({ index }) => {}}/>;
  // @ts-expect-error dragSnapPoints needs drag.
  const undragged = <view aria-label="Sheet" dragSnapPoints={[0, 100]}/>;
  const pager = (at: number, go: (index: number) => void) => <scroll snapIndex={at} onSnapChange={go}/>;
  const detent = (at: number, go: (index: number) => void) => <view drag="y" dragSnapPoints={[0, 200]} snapIndex={at} onSnapChange={go} aria-label="Sheet"/>;
  // @ts-expect-error snapIndex needs onSnapChange.
  const unheard = <scroll snapIndex={0}/>;
  // @ts-expect-error bind and snapIndex both set the snap index.
  const both = (cell: Local<number>) => <scroll bind={cell} snapIndex={0}/>;
  // @ts-expect-error onSnapChange comes with snapIndex.
  const deaf = <scroll onSnapChange={() => {}}/>;
  // @ts-expect-error a <text> takes no container role.
  const textRole = <text role="tablist">x</text>;
  // @ts-expect-error an <image> keeps its own role.
  const imageRole = <image src="a.png" alt="" role="img"/>;
  // @ts-expect-error a <scroll> takes container roles only.
  const scrollRole = <scroll role="tab"/>;
  // @ts-expect-error headingLevel goes on a <view> or <text>.
  const buttonHeading = <button headingLevel={2} onClick={() => {}}>Go</button>;
  // @ts-expect-error a <dialog> keeps its own role.
  const dialogRole = <dialog role="group" aria-label="x"/>;
  // @ts-expect-error aria states are booleans.
  const state = <button aria-pressed="yes">Go</button>;
  // @ts-expect-error unknown style keys are refused.
  const unknownStyle = <view style={{ cursor: 'pointer' }}/>;
  // @ts-expect-error sizeClass is compact or regular.
  const wide = useView().env.sizeClass === 'wide';
  return <Card>
    <text>{view.data.title}{false}{null}{['hello', 1]}</text>
    <input bind={draft}/><textarea bind={draft}/><input type="search" bind={draft}/>
    <checkbox bind={checked}/><switch bind={checked}/>
    <slider bind={volume} aria-label="Volume"/>
    <radio bind={draft} options={[]}/>
    <input value={draft.value} onInput={draft.set}/>
  </Card>;
}

declare const actions: {
  load(params: { id: number }): Promise<{ title: string }>;
  watch(params: { count: number }): Stream<{ token: string }, number>;
  invalid(first: number, second: number): Promise<void>;
};
export function resources() {
  const request = useRequest('record', actions.load, {id:1});
  request.value?.title.toUpperCase();
  // @ts-expect-error unary payload is inferred from the action
  useRequest('bad-record', actions.load, {id:'one'});
  // @ts-expect-error a required payload cannot be omitted
  useRequest('missing-record', actions.load);
  const stream = useStream('tokens', actions.watch, {onData(value) {
    value.token.toUpperCase();
    // @ts-expect-error chunk type is inferred from the action
    value.missing;
  }});
  stream.result?.toFixed();
  stream.start({count:2});
  // @ts-expect-error stream start payload follows the action
  stream.start({count:'two'});
  // @ts-expect-error stream payload is required
  stream.start();
  // @ts-expect-error unary actions cannot become streams
  useStream('wrong-mode', actions.load, {onData() {}});
  // @ts-expect-error page actions accept at most one JSON payload
  useRequest('invalid-arity', actions.invalid, 1, 2);
}

export function localDomain() {
  interface Draft { title: string; note?: string; nested?: { count: number } }
  const draft = useLocal<Draft>('draft', {title:'', note:undefined});
  draft.set({title:'two', nested:undefined});
  useLocal('omitted-fields', {note:undefined, nested:{note:undefined}});
  // @ts-expect-error Date is not JSON-shaped local state
  useLocal('date', new Date());
  // @ts-expect-error functions cannot be local state
  useLocal('function', () => 1);
  // @ts-expect-error nested functions cannot be local state
  useLocal('nested-function', {callback: () => 1});
  // @ts-expect-error undefined is not an array element
  useLocal('array', [undefined]);
  // @ts-expect-error undefined is not a root value
  useLocal('undefined', undefined);
}
interface Payload { items: { title: string }[] }
declare const load: () => Promise<Payload>;
declare const watch: () => Stream<Payload, Payload>;
declare const channel: Channel<Payload>;
declare const updates: (params: { room: string }, channel: ChannelEndpoint<Payload, string>) => void;
export async function readonlyResources() {
  const request = useRequest('load', load);
  // @ts-expect-error request snapshots are frozen
  request.value?.items.push({title:'new'});
  const stream = useStream('watch', watch, {onData(value) {
    // @ts-expect-error resource chunks are frozen
    value.items[0].title = 'new';
  }});
  // @ts-expect-error stream results are frozen
  stream.result?.items.pop();
  useChannel('channel', updates, {params: {room: 'a'}, onData(message) {
    // @ts-expect-error channel messages are frozen
    message.items.pop();
  }});
  // @ts-expect-error channel params follow the handler
  useChannel('wrong-params', updates, {params: {room: 1}});
  const quiet = useStream('quiet', watch);
  quiet.error?.code.toUpperCase();
  const guard = useLeaveGuard('draft', {enabled: true});
  guard.answer(true);
  const current: View = useView();
  current.env.pointer satisfies 'fine' | 'coarse';
  const typedView = useView<{ save(name: string): Promise<boolean>; bad(a: string, b: number): Promise<void>; watch(): AsyncGenerator<number, string, void>; feed: typeof updates }, { count: number }>();
  typedView.data.count satisfies number;
  // @ts-expect-error typed data refuses unknown fields
  typedView.data.missing;
  typedView.actions.save('ok') satisfies Promise<boolean>;
  typedView.actions.watch() satisfies Stream<number, string>;
  useChannel('typed-feed', typedView.actions.feed, {params: {room: 'a'}});
  // @ts-expect-error actions with more than one argument are not callable from the View
  typedView.actions.bad('a', 1);
  const textCell = useLocal('hello');
  textCell.focus();
  const numCell = useLocal(0);
  // @ts-expect-error focus belongs only on string cells an editor can bind
  numCell.focus();
  const boolCell = useLocal(false);
  // @ts-expect-error focus belongs only on string cells an editor can bind
  boolCell.focus();
  const objCell = useLocal({ a: 1 });
  // @ts-expect-error focus belongs only on string cells an editor can bind
  objCell.focus();
  request.stale satisfies boolean;
  const direct = watch();
  // @ts-expect-error direct stream promise results are frozen
  (await direct.result).items.pop();
  direct.on('data', value => {
    // @ts-expect-error direct stream callbacks receive snapshots
    value.items.pop();
  });
  for await (const value of direct) {
    // @ts-expect-error async stream iteration receives snapshots
    value.items.pop();
  }
  for await (const value of channel) {
    // @ts-expect-error async channel iteration receives snapshots
    value.items.pop();
  }
}
function Forward({children}: {children: JSX.Element}) { return children; }
function Optional(): JSX.Element | null { return null; }
export function composition() { return <view><Forward><text>One</text></Forward><Optional/></view>; }
function Empty() { return false; }
export function listComposition() {
  return <view><Empty/><List items={[{id:'one', title:'One'}]} getKey={item => item.id} renderItem={item => <text>{item.title}</text>}/></view>;
}
declare const readonlyView: View<{load(): Promise<Payload>}>;
export async function readonlyAction() {
  // @ts-expect-error direct action results are frozen on the View side
  (await readonlyView.actions.load()).items.pop();
}

export function Joined({ on, size }: { on: boolean; size: 'sm' | 'md' }) {
  const sizes = { sm: 'p-1', md: 'p-2' } as const;
  return <text className={cx('text-sm', sizes[size], on && 'font-bold', { 'opacity-50': !on }, ['grow', null])}>x</text>;
}
// @ts-expect-error cx takes class text, maps and arrays, not functions
cx(() => 'p-1');

// `View<typeof page>`: a `definePage` type supplies data and actions.
type CounterPage = {
  readonly data: { count: number };
  readonly actions: { inc(by: number): Promise<void> };
  readonly __lxPage: { readonly params: {}; readonly result: never; readonly messages: never };
};
export function counter({ data, actions }: View<CounterPage>) {
  const count: number = data.count;
  actions.inc(count);
  // @ts-expect-error action payloads come from the page
  actions.inc('one');
  // @ts-expect-error page data is immutable
  data.count = 2;
  return <text>{String(count)}</text>;
}

// Actions as a page's Logic declares them, as `./$types` hands them over.
type LogicActions = {
  pick(): Promise<{ path: string }>;
  open(input: { path: string }): Promise<void>;
  progress(input: { url: string }): AsyncGenerator<{ fraction: number }, { path: string }>;
};
export function handlers({ actions }: View<LogicActions>) {
  const handed = <button onClick={actions.pick}/>;
  // @ts-expect-error a handler gets no payload; wrap an action that takes one
  const missing = <button onClick={actions.open}/>;
  const wrapped = <button onClick={() => actions.open({ path: 'a.pdf' })}/>;
  const stream = useStream('progress', actions.progress, { onData: step => { const fraction: number = step.fraction; } });
  const path: string | undefined = stream.result?.path;
  return <view>{handed}{missing}{wrapped}<Icon name="M0 0L1 1" onPointer={() => {}} pointerAxis="x"/><Shell popover="manual" closeAfter={10}/><Press drag="x" dragSnapPoints={[0, 40]}/></view>;
}
