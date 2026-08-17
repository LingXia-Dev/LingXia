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

const SHAPE_ONLY = [
  'app',
  'appearance',
  'env',
  'navigationBar',
  'terminal',
  'tray',
] as const satisfies readonly LxApiName[];

const AUTOMATED = [
  { name: 'automation', ownerCaseId: 'AUT-000', requiredLevels: ['boundary'] },
  { name: 'getDeviceInfo', ownerCaseId: 'DEVICE-001', requiredLevels: ['semantic', 'boundary'] },
  { name: 'fs', ownerCaseId: 'LOGIC-004', requiredLevels: ['semantic', 'boundary', 'lifecycle'] },
  { name: 'getLxAppInfo', ownerCaseId: 'LOGIC-001', requiredLevels: ['semantic', 'boundary'] },
  { name: 'getNetworkInfo', ownerCaseId: 'DEVICE-002', requiredLevels: ['semantic', 'boundary'] },
  { name: 'getScreenInfo', ownerCaseId: 'DEVICE-001', requiredLevels: ['semantic', 'boundary'] },
  { name: 'getStorage', ownerCaseId: 'TODO-001', requiredLevels: ['semantic', 'boundary', 'lifecycle'] },
  { name: 'getSystemSetting', ownerCaseId: 'SYSTEM-001', requiredLevels: ['semantic', 'boundary'] },
  { name: 'supports', ownerCaseId: 'LOGIC-006', requiredLevels: ['semantic', 'boundary'] },
  { name: 'getImageInfo', ownerCaseId: 'MEDIA-INFO-001', requiredLevels: ['semantic', 'failure'] },
  { name: 'onDeviceOrientationChange', ownerCaseId: 'LOGIC-002', requiredLevels: ['lifecycle'] },
  { name: 'onKeyDown', ownerCaseId: 'LOGIC-002', requiredLevels: ['lifecycle'] },
  { name: 'onKeyUp', ownerCaseId: 'LOGIC-002', requiredLevels: ['lifecycle'] },
  { name: 'onNetworkChange', ownerCaseId: 'DEVICE-002', requiredLevels: ['lifecycle'] },
  { name: 'onWifiConnected', ownerCaseId: 'LOGIC-002', requiredLevels: ['lifecycle'] },
  {
    name: 'surface',
    ownerCaseId: 'DESKTOP-BROWSER-001',
    requiredLevels: ['semantic', 'lifecycle'],
    requiredTargets: ['windows', 'macos'],
  },
  {
    name: 'shell',
    ownerCaseId: 'DESKTOP-BROWSER-001',
    requiredLevels: ['semantic', 'lifecycle'],
    requiredTargets: ['windows', 'macos'],
  },
  { name: 'createVideoContext', ownerCaseId: 'NATIVE-VIDEO-001', requiredLevels: ['semantic', 'boundary', 'lifecycle'] },
  { name: 'navigateTo', ownerCaseId: 'COMPONENTS-001', requiredLevels: ['semantic', 'boundary', 'lifecycle'] },
  { name: 'navigateBack', ownerCaseId: 'UI-NAV-001', requiredLevels: ['semantic', 'boundary', 'lifecycle'] },
  { name: 'redirectTo', ownerCaseId: 'UI-NAV-001', requiredLevels: ['semantic', 'boundary', 'lifecycle'] },
  { name: 'switchTab', ownerCaseId: 'UI-NAV-001', requiredLevels: ['semantic', 'boundary', 'lifecycle'] },
  { name: 'tabBar', ownerCaseId: 'UI-TABBAR-001', requiredLevels: ['semantic', 'boundary', 'lifecycle'] },
  { name: 'startPullDownRefresh', ownerCaseId: 'PULL-001', requiredLevels: ['semantic', 'lifecycle'] },
  { name: 'stopPullDownRefresh', ownerCaseId: 'PULL-001', requiredLevels: ['semantic', 'lifecycle'] },
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
  'startWifi',
  'stopWifi',
  'vibrateLong',
  'vibrateShort',
] as const satisfies readonly LxApiName[];

const PLANNED = [
  'compressImage',
  'compressVideo',
  'downloadFile',
  'extractVideoThumbnail',
  'getUpdateManager',
  'getVideoInfo',
  'navigateBackApp',
  'navigateToApp',
  'reLaunch',
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
