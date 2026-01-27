export interface PageConfig {
  navigationBarTitleText?: string;
  navigationBarBackgroundColor?: string;
  navigationBarTextStyle?: 'black' | 'white';
  backgroundColor?: string;
  enablePullDownRefresh?: boolean;
  onReachBottomDistance?: number;
}

export interface PageFiles {
  view: {
    path: string;
    exists: boolean;
    type: 'html' | 'react' | 'vue';
  };
  logic: {
    path: string;
    exists: boolean;
  };
  config: {
    path: string;
    exists: boolean;
    data?: PageConfig;
  };
  style: {
    path: string;
    exists: boolean;
  };
}

export interface Page {
  path: string;
  name: string;
  type: 'html' | 'react' | 'vue';
}

export interface BuildOptions {
  dev?: boolean;
  prod?: boolean;
  plugin?: boolean;
  target?: string; // JS target (es5, es2015, es2020, esnext). es5 requires @vitejs/plugin-legacy
}

export interface LxPluginConfig {
  lxPluginId: string;
  version: string;
  main?: string;
  pages: Record<string, string>;
}

export interface BuildResult {
  distDir: string;
  success: boolean;
  error?: string;
}

export interface DependencyConfig {
  react: {
    dependencies: Record<string, string>;
    devDependencies: Record<string, string>;
  };
  vue: {
    dependencies: Record<string, string>;
    devDependencies: Record<string, string>;
  };
}

export interface ProjectStructure {
  projectPath: string;
  outputDir: string;
  pages: Page[];
  hasLogicLayer: boolean;
}
