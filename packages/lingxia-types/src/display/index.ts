/**
 * Display APIs
 * Corresponds to: lingxia-logic/src/display.rs
 */

export type DeviceOrientation = "portrait" | "landscape";

export interface DeviceOrientationChangeEvent {
  value: DeviceOrientation;
}
