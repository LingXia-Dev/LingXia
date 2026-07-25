export enum SurfacePosition {
  Center = 0,
  Bottom = 1,
  Left = 2,
  Right = 3,
  Top = 4
}

export interface SurfaceLayoutConfig {
  id: string;
  appId: string;
  path: string;
  pageInstanceId: string;
  content: number;
  kind: number;
  role: number;
  closeButton: boolean;
  dismissOnOutside: boolean;
  modal: boolean;
  screenWidthVp: number;
  screenHeightVp: number;
  widthVp: number;
  heightVp: number;
  leftVp: number;
  topVp: number;
  position: SurfacePosition;
  bottomInset: number;
  fullWidth: boolean;
  fullHeight: boolean;
  // URL surfaces only: run the Web incognito with a wiped cookie/storage
  // store so each session (auth handoff) starts clean.
  ephemeralWebData: boolean;
  // Callback surfaces never receive local-file access; ordinary browser
  // surfaces may render host-authorized file URLs.
  urlCallback: boolean;
  browserTabId: string;
}
