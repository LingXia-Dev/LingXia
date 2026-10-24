import { useLocal, type Env } from '@lingxia/ui';
import '../../theme/index.css';

export function gradientFixture(env: Env) {
  const cleared = useLocal('gradient-cleared', false);
  return <column className="bg-white p-4 gap-3" style={{paddingTop:env.insets.top + 16}}>
    <button testId="gradient-toggle" className="w-60 h-10 bg-blue-600 text-white" onClick={() => cleared.set(!cleared.value)}>Toggle gradient</button>
    <text testId="gradient-linear" className="w-60 h-16 bg-white bg-linear-to-r/srgb from-[#ff0000] to-[#0000ff] rounded-xl border-2 border-black" style={cleared.value ? {backgroundGradient:'none'} : {}}>gradient-linear</text>
    <text testId="gradient-corner" className="w-60 h-20 bg-linear-to-br/srgb from-[#ff0000] to-[#0000ff]">gradient-corner</text>
    <text testId="gradient-oklab" className="w-60 h-16 bg-linear-to-r from-[#ff0000] to-[#0000ff]">gradient-oklab</text>
    <text testId="gradient-alpha" className="w-60 h-16 bg-white bg-linear-to-r/srgb from-[#ff000000] to-[#0000ff]">gradient-alpha</text>
    <text testId="gradient-hard" className="w-60 h-16 bg-linear-to-r/srgb from-[#ff0000] from-70% to-[#0000ff] to-30%">gradient-hard</text>
  </column>;
}
