/**
 * Location APIs.
 */

export interface GetLocationOptions {
  type?: 'wgs84' | 'gcj02';
  altitude?: boolean;
  isHighAccuracy?: boolean;
  highAccuracyExpireTime?: number;
}

export interface LocationInfo {
  latitude: number;
  longitude: number;
  speed?: number;
  accuracy?: number;
  altitude?: number;
  verticalAccuracy?: number;
  horizontalAccuracy?: number;
}
