import { h, List, useLocal, useRequest, useStream, useChannel, type Channel, type Stream, type Props, type View } from '../../src/index.js';

function Card(props: Props) { return <column {...props}/>; }
export function render(view: View<{ save(value: string): Promise<void> }, { title: string; rows: { text: string }[]; pair: [string, number] }>) {
  const draft = useLocal('draft', '');
  const checked = useLocal('checked', false);
  const volume = useLocal('volume', 1);
  const nested = useLocal('nested', {items: [{title: 'one'}], pair: ['one', 1] as [string, number]});
  nested.set(nested.value);
  nested.set({...nested.value, items: [...nested.value.items, {title: 'two'}]});
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
  const search = <search bind={draft} value="conflict"/>;
  // @ts-expect-error search owns its keyboard
  const keyboard = <search keyboard="email"/>;
  // @ts-expect-error boolean binding has one owner
  const toggle = <switch bind={checked} onChange={() => {}}/>;
  // @ts-expect-error checkbox preserves the binding union
  const checkbox = <checkbox bind={checked} value={false}/>;
  // @ts-expect-error numeric binding has one owner
  const slider = <slider bind={volume} value={1} accessibilityLabel="Volume"/>;
  // @ts-expect-error radio binding has one owner
  const radio = <radio bind={draft} value="a" options={[]}/>;
  // @ts-expect-error objects are not renderable children
  const child = <Card>{{ arbitrary: true }}</Card>;
  // @ts-expect-error native text accepts text, not nested layout nodes
  const nestedText = <text><column/></text>;
  // @ts-expect-error editors are leaves
  const inputChildren = <input><text>invalid</text></input>;
  // @ts-expect-error virtual rows belong to the row factory
  const listChildren = <list count={0} row={() => <text/>}><text/></list>;
  const layout = <view className="flex flex-col md:flex-row" style={{flexDirection:'row-reverse'}}/>;
  const meaningfulImage = <image src="photo.png" alt="A mountain lake"/>;
  const recoverableImage = <image src="photo.png" retryKey="1" onLoad={()=>{}} onError={()=>{}}/>;
  const namedInput = <input accessibilityLabel="Name" bind={draft}/>;
  const decorativeImage = <image src="texture.png" alt=""/>;
  // @ts-expect-error alternative text must be text
  const invalidAlt = <image src="photo.png" alt={3}/>;
  return <Card>
    <text>{view.data.title}{false}{null}{['hello', 1]}</text>
    <input bind={draft}/><textarea bind={draft}/><search bind={draft}/>
    <checkbox bind={checked}/><switch bind={checked}/>
    <slider bind={volume} accessibilityLabel="Volume"/>
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
  useChannel<Payload>('channel', 'updates', {onData(message) {
    // @ts-expect-error channel messages are frozen
    message.items.pop();
  }});
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
export function composition() { return <column><Forward><text>One</text></Forward><Optional/></column>; }
function Empty() { return false; }
export function listComposition() {
  return <column><Empty/><List items={[{id:'one', title:'One'}]} getKey={item => item.id} renderItem={item => <text>{item.title}</text>}/></column>;
}
declare const readonlyView: View<{load(): Promise<Payload>}>;
export async function readonlyAction() {
  // @ts-expect-error direct action results are frozen on the View side
  (await readonlyView.actions.load()).items.pop();
}
