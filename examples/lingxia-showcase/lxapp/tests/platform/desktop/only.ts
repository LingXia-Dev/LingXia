const platform = globalThis.__LINGXIA_AUTOMATION_HOST__?.control?.platform?.toLocaleLowerCase();

/** Why a desktop spec skips on this host: the suite runs every file on every host. */
export const notDesktop: string | undefined =
  !platform || platform === 'macos' || platform === 'windows' ? undefined : 'macOS and Windows only';
