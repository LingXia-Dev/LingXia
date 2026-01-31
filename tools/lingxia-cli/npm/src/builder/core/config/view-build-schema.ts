export interface ViewBuildConfig {
  output: {
    multi: Record<string, unknown>;
  };
  assetDir: string;
  cssCodeSplitMulti: boolean;
  target?: string;
  esbuild?: Record<string, unknown>;
  minifyStrategy?: "esbuild" | "terser" | boolean;
  resolvePlugins?: (framework: "react" | "vue") => Promise<any[]>;
  cssConfig?:
    | false
    | ((
        buildDir: string,
      ) => Promise<{ postcss?: { plugins?: any[] } } | undefined | null>);
}
