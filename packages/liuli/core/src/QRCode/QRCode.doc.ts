import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'QRCode',
  category: 'content',
  status: 'preview',
  since: '2026-11-22',
  summary: 'A code a phone camera reads',
  description:
    'QRCode draws a value as a QR code for a phone camera: a link to open, a network to join, a code to pair with. ' +
    'It is dark on light in both themes, with its quiet zone inside the tile, as readers expect. The value is encoded in Liuli; nothing is fetched.',
  keywords: ['qr', 'qr code', 'barcode', 'scan', 'share', 'pair', 'join'],
  propsType: 'QRCodeProps',
  recipe: { map: 'qrCodeRecipe', parts: 'qrCodeParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The square tile: an image named by `label`.' },
    { part: 'code', required: true, description: 'The modules, dark on light, with a four-module quiet zone.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [],
    target: {},
    contrast: [],
    notes: [
      'It is an image named by `label`, which says what the code is for; its value is not read.',
      'Put the value in words beside it when a person may need to type it instead.',
    ],
  },
  guidance: {
    do: [
      'Say what scanning does in `label`: "Code to join Harbour Café Guest".',
      'Show the link or password beside the code for whoever cannot scan.',
      'Check a value that comes from data with `fitsQRCode` and show it as text when it does not fit.',
      'To save or share the code, give your own button `qrCodeSVG(value, level)`: the same code as an SVG document.',
      'In a `WIFI:` value, put a backslash before every `\\`, `;`, `,`, `"` and `:` in the name and password, and wrap a name or password made only of hex digits in double quotes, or readers take it as hex: `WIFI:T:WPA;S:Harbour Café\\; Upstairs;P:tide\\:pool\\,73;;` joins "Harbour Café; Upstairs" with the password "tide:pool,73".',
    ],
    dont: [
      'Draw it smaller than `sm`; phone cameras read a 96 pt code at arm\'s length.',
      'Raise `level` without a reason; each step makes the code denser.',
    ],
  },
  examples: [
    { name: 'Join', title: 'A network to join' },
    { name: 'Link', title: 'A link to open' },
    { name: 'Sizes', title: 'The three sizes' },
  ],
  gaps: [],
  limits: [
    'Up to version 15: 520 bytes at level L, 412 at M, 292 at Q, 220 at H. A longer value draws an empty tile and warns once; `fitsQRCode(value, level)` answers before rendering.',
    'Byte mode only, so digits and capitals take as much room as any text.',
    'No logo in the middle.',
  ],
  related: ['Thumbnail', 'Code'],
  playground: { props: { value: 'https://example.com', label: 'Code for the example site', size: 'md', level: 'M' } },
  gallery: {
    specimen: 'Specimen',
    tagline: 'A code for a camera',
    order: 20,
    page: {
      lead: 'Use a QR code to hand a link or a code to a phone held up to the screen.',
      hero: 'Join',
      variants: [
        { name: 'Link', title: 'A link, with the address under it' },
        { name: 'Sizes', title: 'Small, medium and large' },
      ],
    },
  },
};
