import { useLocal, type Env } from '@lingxia/ui';
import '../../theme/index.css';

const alignments = ['right', 'left', 'center', 'start', 'end', 'justify'] as const;
const classes = { right: 'text-right', left: 'text-left', center: 'text-center', start: 'text-start', end: 'text-end', justify: 'text-justify' };
export function alignmentFixture(env: Env) {
  const mode = useLocal('alignment-mode', 0);
  const value = useLocal('alignment-input', 'MMMM');
  const alignment = alignments[mode.value];
  return <column className="bg-white p-4 gap-3 text-black" style={{ paddingTop: env.insets.top + 16 }}>
    <button testId="alignment-toggle" onClick={() => mode.set((mode.value + 1) % alignments.length)} className="w-60 h-12 bg-blue-600 text-white">{alignment}</button>
    <column className={`gap-3 ${classes[alignment]}`}>
      <text testId="alignment-lines" className="w-60 h-20 p-2 border-2 border-black text-xl">{'MMMM\nM'}</text>
      <text testId="alignment-rtl" className="w-60 h-20 p-2 border-2 border-black text-xl">{'\u05d0\u05d1\u05d2\n\u05d0'}</text>
      <input testId="alignment-input" bind={value} className="w-60 h-14 p-2 border-2 border-black rounded-none text-xl bg-white" />
      <text testId="alignment-override" className="w-60 h-12 p-2 border-2 border-black text-left text-xl">MMMM</text>
    </column>
    <text testId="alignment-status" className="text-sm">{value.value}</text>
  </column>;
}
