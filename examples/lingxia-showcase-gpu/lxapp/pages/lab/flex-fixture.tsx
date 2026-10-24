import { useLocal, type Env } from '@lingxia/ui';

export function flexFixture(env: Env) {
  const narrow = useLocal('flex-narrow', false);
  return <column className="bg-white text-black" style={{paddingTop:env.insets.top,paddingLeft:8}}>
    <button testId="flex-resize" onClick={()=>narrow.set(!narrow.value)} className="h-11 bg-blue-500 text-white">Resize</button>
    <column className="gap-3" style={{width:narrow.value?240:300}}>
      <row><text testId="flex-auto-a" className="w-[50px] h-6 grow bg-red-100">A</text><text testId="flex-auto-b" className="w-[100px] h-6 grow bg-blue-100">B</text></row>
      <row><text testId="flex-zero-a" className="w-[50px] h-6 flex-1 bg-red-100">A</text><text testId="flex-zero-b" className="w-[100px] h-6 flex-1 bg-blue-100">B</text></row>
      <row><text testId="flex-half" className="basis-1/2 h-6 shrink-0 bg-green-100">Half</text><text className="basis-12 h-6 shrink-0">48</text></row>
      <column className="h-[100px]">
        <text testId="flex-column-a" className="h-[20px] grow bg-red-100">A</text>
        <text testId="flex-column-b" className="h-[40px] grow bg-blue-100">B</text>
      </column>
    </column>
  </column>;
}
