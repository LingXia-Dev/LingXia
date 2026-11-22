import {List, useLocal} from '@lingxia/view';

export function render() {
  const attempt=useLocal('attempt',0), status=useLocal('status','loading');
  const slow=useLocal('slow',true), order=useLocal('order',['a','b']);
  return <view style={{padding:12,gap:8}}>
    <text data-testid="image-state">{status.value}</text>
    <image src="retry.png" alt="" retryKey={String(attempt.value)} onLoad={()=>status.set('ready')} onError={()=>status.set('failed')} style={{width:32,height:32}}/>
    <button data-testid="retry" onClick={()=>{status.set('loading');attempt.set(attempt.value+1);}}>Retry</button>
    {slow.value && <image src="slow.png" alt="" style={{width:32,height:32}}/>}
    <button data-testid="remove-slow" onClick={()=>slow.set(false)}>Remove slow image</button>
    <button data-testid="reorder" onClick={()=>order.set([...order.value].reverse())}>Reorder</button>
    <List data-testid="keyed-list" items={order.value} getKey={key=>key} style={{height:100}} renderItem={key=>{
      const n=useLocal('count',0);
      return <button data-testid={`item-${key}`} onClick={()=>n.set(n.value+1)}>{`${key}:${n.value}`}</button>;
    }}/>
    <List data-testid="padded-list" items={Array.from({length:100},(_,i)=>i)} getKey={String} anchor="end" className="pt-[7px] pb-[19px] pl-[23px] pr-[11px] border-[3px]" style={{height:100}} renderItem={i=><text data-testid={`row-${i}`} style={{height:40}}>{i}</text>}/>
  </view>;
}
