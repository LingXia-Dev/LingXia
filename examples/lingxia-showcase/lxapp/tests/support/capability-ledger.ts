import { LX_API_NAMES } from '../api/manifest.js';

export type CoverageTarget = 'windows' | 'macos' | 'android';
export type CoverageMode = 'automated' | 'external-fixture' | 'planned' | 'shape-only';
export type RequiredCoverageLevel = 'semantic' | 'failure' | 'boundary' | 'lifecycle';
export type TargetOutcome =
  | 'supported'
  | 'absent'
  | 'reject'
  | 'no-op'
  | 'external-ui'
  | 'supported-or-absent'
  | 'mixed';

type LxApiName = (typeof LX_API_NAMES)[number];

export interface CapabilityRequirement {
  capability: `lx.${LxApiName}`;
  mode: CoverageMode;
  expectedOutcome: Record<CoverageTarget, TargetOutcome>;
  ownerCaseId?: string;
  requiredLevels?: readonly RequiredCoverageLevel[];
  requiredTargets?: readonly CoverageTarget[];
}

const allTargets = (outcome: TargetOutcome): Record<CoverageTarget, TargetOutcome> => ({
  android: outcome,
  macos: outcome,
  windows: outcome,
});

const SHAPE_ONLY = ['app', 'appearance', 'env', 'navigationBar', 'shell', 'tabBar', 'tray'] as const satisfies readonly LxApiName[];

const AUTOMATED = [
  { name: 'automation', ownerCaseId: 'AUT-000', requiredLevels: ['boundary'] },
  { name: 'getDeviceInfo', ownerCaseId: 'LOGIC-001', requiredLevels: ['semantic', 'boundary'] },
  { name: 'getFileManager', ownerCaseId: 'LOGIC-004', requiredLevels: ['semantic', 'boundary', 'lifecycle'] },
  { name: 'getLxAppInfo', ownerCaseId: 'LOGIC-001', requiredLevels: ['semantic', 'boundary'] },
  { name: 'getNetworkInfo', ownerCaseId: 'LOGIC-001', requiredLevels: ['semantic', 'boundary'] },
  { name: 'getScreenInfo', ownerCaseId: 'LOGIC-001', requiredLevels: ['semantic', 'boundary'] },
  { name: 'getStorage', ownerCaseId: 'LOGIC-003', requiredLevels: ['semantic', 'boundary', 'lifecycle'] },
  { name: 'getSystemSetting', ownerCaseId: 'LOGIC-001', requiredLevels: ['semantic', 'boundary'] },
  { name: 'offDeviceOrientationChange', ownerCaseId: 'LOGIC-002', requiredLevels: ['lifecycle'] },
  { name: 'offKeyDown', ownerCaseId: 'LOGIC-002', requiredLevels: ['lifecycle'] },
  { name: 'offKeyUp', ownerCaseId: 'LOGIC-002', requiredLevels: ['lifecycle'] },
  { name: 'offNetworkChange', ownerCaseId: 'LOGIC-002', requiredLevels: ['lifecycle'] },
  { name: 'offWifiConnected', ownerCaseId: 'LOGIC-002', requiredLevels: ['lifecycle'] },
  { name: 'onDeviceOrientationChange', ownerCaseId: 'LOGIC-002', requiredLevels: ['lifecycle'] },
  { name: 'onKeyDown', ownerCaseId: 'LOGIC-002', requiredLevels: ['lifecycle'] },
  { name: 'onKeyUp', ownerCaseId: 'LOGIC-002', requiredLevels: ['lifecycle'] },
  { name: 'onNetworkChange', ownerCaseId: 'LOGIC-002', requiredLevels: ['lifecycle'] },
  { name: 'onWifiConnected', ownerCaseId: 'LOGIC-002', requiredLevels: ['lifecycle'] },
  {
    name: 'openSurface',
    ownerCaseId: 'DESKTOP-BROWSER-001',
    requiredLevels: ['semantic', 'lifecycle'],
    requiredTargets: ['windows', 'macos'],
  },
] as const satisfies ReadonlyArray<{
  name: LxApiName;
  ownerCaseId: string;
  requiredLevels: readonly RequiredCoverageLevel[];
  requiredTargets?: readonly CoverageTarget[];
}>;

const EXTERNAL_FIXTURE = [
  'chooseDirectory',
  'chooseFile',
  'chooseMedia',
  'connectWifi',
  'getConnectedWifi',
  'getLocation',
  'getWifiList',
  'hideToast',
  'makePhoneCall',
  'openExternal',
  'openFile',
  'previewMedia',
  'saveImageToPhotosAlbum',
  'saveVideoToPhotosAlbum',
  'scanCode',
  'setDeviceOrientation',
  'setMoreActions',
  'share',
  'showActionSheet',
  'showModal',
  'showToast',
  'startPullDownRefresh',
  'startWifi',
  'stopPullDownRefresh',
  'stopWifi',
  'vibrateLong',
  'vibrateShort',
] as const satisfies readonly LxApiName[];

const PLANNED = [
  'compressImage',
  'compressVideo',
  'createVideoContext',
  'downloadFile',
  'extractVideoThumbnail',
  'getImageInfo',
  'getUpdateManager',
  'getVideoInfo',
  'navigateBack',
  'navigateBackApp',
  'navigateTo',
  'navigateToApp',
  'onSurfaceContext',
  'reLaunch',
  'redirectTo',
  'switchTab',
  'uploadFile',
] as const satisfies readonly LxApiName[];

export const LX_CAPABILITY_LEDGER: readonly CapabilityRequirement[] = [
  ...SHAPE_ONLY.map((name) => ({
    capability: `lx.${name}` as const,
    expectedOutcome: allTargets('supported'),
    mode: 'shape-only' as const,
  })),
  ...AUTOMATED.map(({ name, ownerCaseId, requiredLevels, ...requirement }) => ({
    capability: `lx.${name}` as const,
    expectedOutcome: allTargets('supported'),
    mode: 'automated' as const,
    ownerCaseId,
    requiredLevels,
    ...requirement,
  })),
  ...EXTERNAL_FIXTURE.map((name) => ({
    capability: `lx.${name}` as const,
    expectedOutcome: allTargets('external-ui'),
    mode: 'external-fixture' as const,
  })),
  ...PLANNED.map((name) => ({
    capability: `lx.${name}` as const,
    expectedOutcome: allTargets('supported-or-absent'),
    mode: 'planned' as const,
  })),
];

export function capabilityLedgerIssues(): {
  duplicates: string[];
  missing: string[];
  unknown: string[];
} {
  const canonical = new Set(LX_API_NAMES.map((name) => `lx.${name}`));
  const counts = new Map<string, number>();
  for (const { capability } of LX_CAPABILITY_LEDGER) {
    counts.set(capability, (counts.get(capability) ?? 0) + 1);
  }
  return {
    duplicates: Array.from(counts).filter(([, count]) => count > 1).map(([name]) => name),
    missing: Array.from(canonical).filter((name) => !counts.has(name)),
    unknown: Array.from(counts.keys()).filter((name) => !canonical.has(name)),
  };
}
