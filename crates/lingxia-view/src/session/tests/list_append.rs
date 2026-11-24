use super::*;
use rong::{JSEngine, Source};

// Appending a page of rows to a long List, timed in QuickJS: the cost must not grow with the list.
const BENCH: &str = r#"(() => {
  const row = i => ({ kind: 'visit', key: `r${i}`, id: `r${i}`, url: `https://example.com/${i}`, title: `Row ${i}`, host: 'example.com', at: 1700000000 + i });
  const env = { viewport: { width: 400, height: 800 } };
  const lines = [];
  for (const size of [400, 1000, 2000, 3000, 5000]) {
    let revision = 1;
    let view = __lingxiaCreateView({ rows: Array.from({ length: size }, (_, i) => row(i)) }, env, revision);
    __lingxiaPrepare(__lingxiaRender, view); __lingxiaCommit();
    const sync = [], frame = [];
    for (let round = 0; round < 7; round++) {
      const base = size + round * 200;
      const ops = Array.from({ length: 200 }, (_, i) => ({ op: 'add', path: '/rows/-', value: row(base + i) }));
      const t0 = Date.now();
      __lingxiaSyncData(revision, revision + 1, ops); revision++;
      view = __lingxiaCreateView(undefined, env);
      const t1 = Date.now();
      __lingxiaPrepare(__lingxiaRender, view); __lingxiaCommit();
      sync.push(t1 - t0); frame.push(Date.now() - t1);
    }
    const median = list => list.sort((a, b) => a - b)[3];
    lines.push(`${size}: sync ${median(sync)} ms, render+encode ${median(frame)} ms`);
  }
  const big = Object.freeze(Array.from({ length: 5000 }, (_, i) => i));
  let t = Date.now(); for (let i = 0; i < 20; i++) Object.isFrozen(big); lines.push(`isFrozen(5000) x20: ${Date.now() - t} ms`);
  t = Date.now(); for (let i = 0; i < 20; i++) Object.freeze(big.slice()); lines.push(`freeze(slice 5000) x20: ${Date.now() - t} ms`);
  t = Date.now(); for (let i = 0; i < 20; i++) big.slice(); lines.push(`slice(frozen 5000) x20: ${Date.now() - t} ms`);
  return lines.join('\n');
})()"#;

#[test]
#[ignore = "measurement: cargo test -p lingxia-view --lib list_append -- --ignored --nocapture"]
fn list_append_cost_in_quickjs() {
    let module = source(
        "function render(view){return h('page',{},h(List,{items:view.data.rows,getKey:r=>r.key,getItemHeight:()=>44,renderItem:r=>h('text',{style:{height:44}},r.title),style:{height:800}}))}",
    );
    let runtime = crate::engine::RongJS::runtime();
    let context = runtime.context();
    crate::engine::compatibility::install(&context).unwrap();
    context
        .eval::<()>(Source::from_bytes(module.as_bytes()))
        .unwrap();
    let report: String = context.eval(Source::from_bytes(BENCH.as_bytes())).unwrap();
    println!("{report}");
}
