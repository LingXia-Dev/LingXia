import {useLocal} from '@lingxia/ui';

export function render() {
  const attempt=useLocal('attempt',0), status=useLocal('status','loading');
  const slow=useLocal('slow',true), order=useLocal('order',['a','b']);
  return <column style={{padding:12,gap:8}}>
    <text testId="image-state">{status.value}</text>
    <image src="retry.png" retryKey={String(attempt.value)} onLoad={()=>status.set('ready')} onError={()=>status.set('failed')} style={{width:32,height:32}}/>
    <button testId="retry" onClick={()=>{status.set('loading');attempt.set(attempt.value+1);}}>Retry</button>
    {slow.value && <image src="slow.png" style={{width:32,height:32}}/>}
    <button testId="remove-slow" onClick={()=>slow.set(false)}>Remove slow image</button>
    <button testId="reorder" onClick={()=>order.set([...order.value].reverse())}>Reorder</button>
    <list testId="keyed-list" count={order.value.length} itemKeys={order.value} style={{height:100}} row={i=>{
      const n=useLocal('count',0), key=order.value[i];
      return <button key={key} testId={`item-${key}`} onClick={()=>n.set(n.value+1)}>{`${key}:${n.value}`}</button>;
    }}/>
    <list testId="padded-list" count={100} anchor="end" className="pt-[7px] pb-[19px] pl-[23px] pr-[11px] border-[3px]" style={{height:100}} row={i=><text testId={`row-${i}`} style={{height:40}}>{i}</text>}/>
  </column>;
}
