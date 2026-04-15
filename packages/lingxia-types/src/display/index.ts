/**
 * Display and orientation APIs.
 */

export type DeviceOrientation = "portrait" | "landscape";

export interface DeviceOrientationChangeEvent {
  value: DeviceOrientation;
}
