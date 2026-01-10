// MARK: - SurfaceHost
// Surface host for managing XComponent surfaces

import { BackendKind } from './Contract';

/**
 * Token representing a surface for video rendering.
 * Used to prevent stale surface destroy callbacks from affecting new surfaces.
 */
export class SurfaceToken {
  /** Platform surface identifier (XComponent surface ID) */
  readonly id: string;

  /** Rebuild count - increments each time surface is recreated for the same ownerKey */
  readonly generation: number;

  /** Component instance key: "{pageId}/{componentId}" */
  readonly ownerKey: string;

  constructor(id: string, generation: number, ownerKey: string) {
    this.id = id;
    this.generation = generation;
    this.ownerKey = ownerKey;
  }

  /** Check if this token belongs to the same owner */
  isSameOwner(other: SurfaceToken): boolean {
    return this.ownerKey === other.ownerKey;
  }

  /** Check if this token is newer than another (same owner, higher generation) */
  isNewerThan(other: SurfaceToken): boolean {
    return this.ownerKey === other.ownerKey && this.generation > other.generation;
  }

  equals(other: SurfaceToken): boolean {
    return this.id === other.id &&
           this.generation === other.generation &&
           this.ownerKey === other.ownerKey;
  }
}

/**
 * Host for managing video rendering surfaces.
 * Handles XComponent surface lifecycle for both URL and Feed playback.
 */
export class SurfaceHost {
  private readonly ownerKey: string;
  private generation: number = 0;

  /** The current surface ID for playback */
  private _surfaceId: string = '';

  /** Current active backend */
  private _activeBackend: BackendKind | undefined = undefined;

  get surfaceId(): string {
    return this._surfaceId;
  }

  get activeBackend(): BackendKind | undefined {
    return this._activeBackend;
  }

  constructor(ownerKey: string) {
    this.ownerKey = ownerKey;
  }

  /**
   * Creates a new surface token with incremented generation
   */
  createToken(): SurfaceToken {
    this.generation++;
    return new SurfaceToken(this._surfaceId, this.generation, this.ownerKey);
  }

  /**
   * Gets the current surface token without incrementing generation
   */
  getCurrentToken(): SurfaceToken | undefined {
    if (this.generation === 0) return undefined;
    return new SurfaceToken(this._surfaceId, this.generation, this.ownerKey);
  }

  /**
   * Sets the surface ID
   */
  setSurfaceId(surfaceId: string): void {
    this._surfaceId = surfaceId;
  }

  /**
   * Sets the active backend
   */
  setActiveBackend(backend: BackendKind | undefined): void {
    this._activeBackend = backend;
  }

  /**
   * Validates if a token is still current (prevents stale callbacks)
   */
  isTokenCurrent(token: SurfaceToken): boolean {
    return token.ownerKey === this.ownerKey && token.generation === this.generation;
  }

  /**
   * Clears the surface state
   */
  clear(): void {
    this._surfaceId = '';
  }
}
