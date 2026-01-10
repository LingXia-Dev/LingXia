// MARK: - JsEventMapper
// Maps PlayerEvent to JS event detail objects

import {
  PlayerEvent,
  PlayerEventType,
  PlayingEvent,
  PauseEvent,
  WaitingEvent,
  SeekingEvent,
  SeekedEvent,
  TimeUpdateEvent,
  LoadedMetadataEvent,
  EndedEvent,
  ErrorEvent,
  RateChangeEvent,
  VolumeChangeEvent,
  StopEvent,
  FullscreenChangeEvent,
  getEventName
} from './PlayerEvent';

export class JsEventPayload {
  name: string;
  detail: Map<string, Object>;

  constructor(name: string, detail: Map<string, Object>) {
    this.name = name;
    this.detail = detail;
  }
}

/**
 * Converts milliseconds to seconds.
 * Returns 0 if undefined.
 */
function msToSeconds(ms: number | undefined): number {
  return ms !== undefined ? ms / 1000 : 0;
}

/**
 * Maps a PlayerEvent to a JS-compatible event payload.
 * Time values are converted from milliseconds to seconds.
 */
export function mapPlayerEventToJs(event: PlayerEvent): JsEventPayload {
  const name = getEventName(event);
  const detail = new Map<string, Object>();

  switch (event.type) {
    case PlayerEventType.PLAY_REQUEST:
    case PlayerEventType.PLAY:
      break;

    case PlayerEventType.PLAYING: {
      const e = event as PlayingEvent;
      detail.set('currentTime', msToSeconds(e.currentTimeMs));
      break;
    }

    case PlayerEventType.PAUSE: {
      const e = event as PauseEvent;
      detail.set('currentTime', msToSeconds(e.currentTimeMs));
      break;
    }

    case PlayerEventType.WAITING: {
      const e = event as WaitingEvent;
      detail.set('reason', e.reason);
      detail.set('currentTime', msToSeconds(e.currentTimeMs));
      break;
    }

    case PlayerEventType.SEEKING: {
      const e = event as SeekingEvent;
      detail.set('targetTime', msToSeconds(e.targetTimeMs));
      detail.set('currentTime', msToSeconds(e.currentTimeMs));
      break;
    }

    case PlayerEventType.SEEKED: {
      const e = event as SeekedEvent;
      detail.set('currentTime', msToSeconds(e.currentTimeMs));
      break;
    }

    case PlayerEventType.TIME_UPDATE: {
      const e = event as TimeUpdateEvent;
      detail.set('currentTime', msToSeconds(e.currentTimeMs));
      detail.set('duration', msToSeconds(e.durationMs));
      break;
    }

    case PlayerEventType.LOADED_METADATA: {
      const e = event as LoadedMetadataEvent;
      detail.set('duration', msToSeconds(e.durationMs));
      detail.set('width', e.width);
      detail.set('height', e.height);
      break;
    }

    case PlayerEventType.ENDED: {
      const e = event as EndedEvent;
      detail.set('currentTime', msToSeconds(e.currentTimeMs));
      break;
    }

    case PlayerEventType.ERROR: {
      const e = event as ErrorEvent;
      detail.set('code', e.code);
      detail.set('message', e.message);
      detail.set('nativeCode', e.nativeCode ?? '');
      detail.set('httpStatus', e.httpStatus ?? 0);
      detail.set('retryable', e.retryable ?? false);
      detail.set('backend', e.backend ?? '');
      break;
    }

    case PlayerEventType.RATE_CHANGE: {
      const e = event as RateChangeEvent;
      detail.set('rate', e.rate);
      break;
    }

    case PlayerEventType.VOLUME_CHANGE: {
      const e = event as VolumeChangeEvent;
      detail.set('volume', e.volume);
      detail.set('muted', e.muted);
      break;
    }

    case PlayerEventType.STOP: {
      const e = event as StopEvent;
      detail.set('reason', e.reason);
      break;
    }

    case PlayerEventType.FULLSCREEN_CHANGE: {
      const e = event as FullscreenChangeEvent;
      detail.set('fullscreen', e.fullscreen);
      break;
    }

    default:
      break;
  }

  return new JsEventPayload(name, detail);
}
