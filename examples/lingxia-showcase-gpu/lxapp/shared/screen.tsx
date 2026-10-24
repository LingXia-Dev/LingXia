import { type Env, type Props } from '@lingxia/ui';

export function colors(env: Env) {
  const dark = env.theme === 'dark';
  return {
    background: dark ? '#121C30' : '#F6F7FB',
    surface: dark ? '#202D45' : '#FFFFFF',
    ink: dark ? '#F1F5FF' : '#18243A',
    muted: dark ? '#A8B5CF' : '#63718A',
    accent: dark ? '#ABBBFF' : '#435DD9',
  };
}

export function Screen({ subtitle, env, children, style, scrollId = 'page-scroll' }: Pick<Props, 'children' | 'style'> & {
  subtitle: string; env: Env; scrollId?: string;
}) {
  const c = colors(env);
  return <column style={{ background: c.background, ...style }}>
    <spacer style={{ height: env.insets.top }} />
    <row style={{ grow: 1, flexBasis: 0, justify: 'center' }}>
      <spacer style={{ width: env.insets.left }} />
      <column style={{ grow: 1, flexBasis: 0, maxWidth: 720, padding: 18, gap: 14 }}>
        <scroll testId={scrollId} style={{ grow: 1, flexBasis: 0, gap: 14 }}>
          <text style={{ fontSize: 16, lineHeight: 24, color: c.muted }}>{subtitle}</text>
          {children}
        </scroll>
      </column>
      <spacer style={{ width: env.insets.right }} />
    </row>
    <spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} />
  </column>;
}
