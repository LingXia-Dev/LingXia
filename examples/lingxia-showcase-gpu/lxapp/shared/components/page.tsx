import { useView, type Env, type Props, type Style } from '@lingxia/ui';

export function contentBounds(env: Env) {
  // Insets are overlap within the measured viewport; resized chrome is already
  // outside it. Keyboard and bottom chrome overlap, so consume their maximum.
  return {
    width: Math.max(1, env.viewport.width - env.insets.left - env.insets.right),
    height: Math.max(1, env.viewport.height - env.insets.top - Math.max(env.insets.bottom, env.keyboard.height)),
  };
}

export interface PageContentProps extends Props {
  /** Layout inside the safe area. Viewport dimensions remain owned by PageContent. */
  contentStyle?: Style;
  contentClassName?: string;
}

export function PageContent({ contentStyle, contentClassName, ...props }: PageContentProps) {
  const { env } = useView();
  const bounds = contentBounds(env);
  return <column {...props} style={{ ...props.style, width: env.viewport.width, height: env.viewport.height }}>
    <spacer style={{ height: env.insets.top }}/>
    <row style={{ height: bounds.height }}>
      <spacer style={{ width: env.insets.left }}/>
      <column className={contentClassName} style={{ ...contentStyle, ...bounds }}>{props.children}</column>
      <spacer style={{ width: env.insets.right }}/>
    </row>
    <spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }}/>
  </column>;
}
