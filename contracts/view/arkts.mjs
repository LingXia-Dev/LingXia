// ArkTS decoders of the frame contract for the HarmonyOS SDK. ArkTS forbids `any`,
// index signatures and untyped object literals, so every frame type is an interface
// whose fields are what `JSON.parse(...) as T` yields, and enums are string constants.

const header = '// Generated from contracts/view/schema.json; do not edit.\n';
const camel = key => key.replace(/[-_]([a-z0-9])/g, (_, c) => c.toUpperCase());
const constant = name => name.replace(/-/g, '_').replace(/([a-z0-9])([A-Z])/g, '$1_$2').toUpperCase().replace(/__+/g, '_');

function frameType(ty) {
  let optional = false, dims = 0;
  if (ty.endsWith('?')) { optional = true; ty = ty.slice(0, -1); }
  for (let m; (m = ty.match(/\[(\d*)\]$/)); ty = ty.slice(0, m.index)) dims += 1;
  return { base: ty, optional, dims };
}

/** `GpuFrame.ets`: one interface per contract type; JSON keys are kept, since `as` casts do no renaming. */
export function frameArkTS(schema) {
  const { types, messages } = schema.frame;
  const prim = { string: 'string', bool: 'boolean', u8: 'number', u32: 'number', u64: 'number', usize: 'number', i64: 'number', f32: 'number', f64: 'number', any: 'Object' };
  const tsType = ty => {
    const t = frameType(ty);
    let element = prim[t.base] ?? (types[t.base]?.enum ? 'string' : `GpuFrame${t.base}`);
    for (let i = 0; i < t.dims; i++) element = `${element}[]`;
    return { type: element, optional: t.optional };
  };
  const fieldsOf = def => def.tagged
    ? Object.fromEntries([[def.tagged, 'string'], ...Object.values(def.variants).flatMap(fields => Object.entries(fields).map(([k, v]) => [k, v.endsWith('?') ? v : `${v}?`]))])
    : def;
  const interfaces = Object.entries(types).filter(([, def]) => !def.enum).map(([name, def]) => {
    const fields = Object.entries(fieldsOf(def)).filter(([key]) => key !== '...').map(([key, ty]) => {
      const { type, optional } = tsType(ty);
      return optional ? `  ${key}?: ${type} | null;` : `  ${key}: ${type};`;
    });
    return `/** \`${name}\` in the frame contract. */\nexport interface GpuFrame${name} {\n${fields.join('\n')}\n}`;
  });
  const enums = Object.entries(types).filter(([, def]) => def.enum).map(([name, def]) =>
    `/** \`${name}\` values. */\nexport class GpuFrame${name}Values {\n${def.enum.map(value => `  static readonly ${constant(camel(value))}: string = ${JSON.stringify(value)};`).join('\n')}\n}`);
  const tags = Object.entries(types).filter(([, def]) => def.tagged).map(([name, def]) =>
    `/** \`${name}\` variants, by its \`${def.tagged}\` key. */\nexport class GpuFrame${name}Kinds {\n${Object.keys(def.variants).map(variant => `  static readonly ${constant(camel(variant))}: string = ${JSON.stringify(variant)};`).join('\n')}\n}`);
  return `${header}// The frame contract the Harmony SDK decodes: ${Object.entries(messages).map(([name, type]) => `${name} = ${type}`).join(', ')}.

${[...enums, ...tags, ...interfaces].join('\n\n')}
`;
}

/** `GpuInput.ets`: the core's editor limits, which an ArkTS input mirror holds itself to. */
export function inputArkTS(schema) {
  return `${header}export class GpuInputLimits {
${['editorTextBytes', 'inputRequestBytes', 'inputBatchEdits', 'clipboardItems'].map(name => `  static readonly ${constant(name)}: number = ${schema.limits[name]};`).join('\n')}
}
`;
}

/** Repository-relative outputs, for `generate()`. */
export function arkts(schema) {
  const root = 'lingxia-sdk/harmony/lingxia/src/main/ets/view/generated';
  return {
    [`${root}/GpuFrame.ets`]: frameArkTS(schema),
    [`${root}/GpuInput.ets`]: inputArkTS(schema),
  };
}
