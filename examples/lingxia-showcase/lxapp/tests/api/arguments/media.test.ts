import { argumentContracts, type Row } from './contract.js';

const dir = 'lx://userdata/{ns}';
const png = `${dir}/a.png`;
const missing = `${dir}/missing.png`;
const missingVideo = `${dir}/missing.mp4`;
// A 1x1 RGB PNG.
const PNG_BASE64 = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC';
const seed: Row = ['seed png', [png, { data: PNG_BASE64, encoding: 'base64' }], 'ok', { via: 'fs.write' }];
// Text under an image name: a content error, not a device limit.
const text = `${dir}/note.png`;
const seedText: Row = ['seed text', [text, { data: 'not an image' }], 'ok', { via: 'fs.write' }];
// Neither a picture nor a video by name or bytes: refused before anything is saved.
const note = `${dir}/note.txt`;
const seedNote: Row = ['seed note', [note, { data: 'not media' }], 'ok', { via: 'fs.write' }];
const reset = [['fs.remove', [dir, { recursive: true }]]] as const;

const pathRows = (target: string): readonly Row[] => [
  ['no path', [], 'E_INVALID_ARG'],
  ['number path', [1], 'E_INVALID_ARG'],
  ['empty path', [''], 'E_INVALID_ARG'],
  ['native absolute', ['/etc/hosts'], 'E_INVALID_ARG'],
  ['missing file', [target], 'E_NOT_FOUND'],
];

argumentContracts({
  'media.imageInfo': {
    rows: [...pathRows(missing), seed, ['png', [png], 'ok'], seedText, ['not an image', [text], 'E_INVALID_DATA']],
    cleanup: reset,
  },
  'media.compressImage': {
    rows: [
      ...pathRows(missing),
      seed,
      ['options not object', [png, 'x'], 'E_INVALID_ARG'],
      ['string quality', [png, { quality: 'x' }], 'E_INVALID_ARG'],
      ['png', [png, { quality: 50 }], 'ok'],
      seedText,
      ['not an image', [text], 'E_INVALID_DATA'],
    ],
    cleanup: reset,
  },
  'media.videoInfo': {
    rows: [...pathRows(missingVideo), seed, ['not a video', [png], 'E_INVALID_DATA']],
    cleanup: reset,
  },
  'media.videoThumbnail': {
    rows: [
      ...pathRows(missingVideo),
      ['options not object', [missingVideo, 'x'], 'E_INVALID_ARG'],
      seed,
      ['not a video', [png], 'E_INVALID_DATA'],
    ],
    cleanup: reset,
  },
  'media.compressVideo': {
    rows: [
      ...pathRows(missingVideo),
      ['options not object', [missingVideo, 'x'], 'E_INVALID_ARG'],
      seed,
      ['quality with fps', [png, { quality: 'low', fps: 10 }], 'E_INVALID_ARG'],
      ['not a video', [png], 'E_INVALID_DATA', { then: 'result' }],
    ],
    cleanup: reset,
  },
  // A file that is there would be saved to the person's photo library.
  'media.saveToLibrary': {
    feature: 'media.saveToLibrary',
    refuseOnly: true,
    rows: [...pathRows(missing), seedNote, ['not media', [note], 'E_INVALID_DATA']],
    cleanup: reset,
  },
  // A valid source opens the viewer.
  'media.preview': {
    refuseOnly: true,
    rows: [
      ['no options', [], 'E_INVALID_ARG'],
      ['options not path or object', [1], 'E_INVALID_ARG'],
      ['no path', [{}], 'E_INVALID_ARG'],
      ['empty sources', [{ sources: [] }], 'E_INVALID_ARG'],
      ['number source path', [{ sources: [{ path: 1 }] }], 'E_INVALID_ARG'],
    ],
  },
});
