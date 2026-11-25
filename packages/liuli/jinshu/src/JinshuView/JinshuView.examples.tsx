import { Said } from '@liulidesign/core/internal/example';
import { audio, callout, codeBlock, createDocument, details, fileBlock, heading, image, list, listItem, mathBlock, paragraph, quote, run, table, video } from '../schema/helpers';
import { JinshuView } from './JinshuView';

// Big Buck Bunny (c) Blender Foundation, CC BY 3.0: a public 10 s clip, as core's video examples use.
const clip = 'https://test-videos.co.uk/vids/bigbuckbunny/mp4/h264/720/Big_Buck_Bunny_720_10s_1MB.mp4';
const poster = 'https://peach.blender.org/wp-content/uploads/title_anouncement.jpg';

const lesson = createDocument({
  title: 'Limits',
  blocks: [
    heading(2, 'What a limit says', 'h1'),
    paragraph([run('A limit describes where a function '), run('heads', { bold: true }), run(', not where it is.')], 'p1'),
    image('public/photos/photo-1.png', { alt: 'The sea running out to the horizon at sunset', width: 480, height: 360, caption: 'The horizon: the sea heads toward it and never arrives.', id: 'g1' }),
    mathBlock('\\lim_{x \\to 1} \\frac{x^2 - 1}{x - 1} = 2', 'm1'),
    list('task', [listItem('Read the definition', { id: 'i1', checked: true }), listItem('Try the exercises', { id: 'i2', checked: false })], { id: 'l1' }),
    callout('tip', [paragraph('Sketch the graph first.', 'p2')], { title: 'Tip', id: 'c1' }),
    table(['Notation', 'Read as', 'Means'], [['lim x→a f(x)', 'the limit of f at a', 'where f heads near a'], ['x→a⁺', 'from the right', 'only x greater than a']], { id: 't1' }),
    quote([paragraph('The limit does not care what happens at the point itself.', 'q1p')], 'q1'),
    details('Why the hollow point?', [paragraph('Dividing by x − 1 is undefined at 1, so the graph has a hole there.', 'd1p')], { id: 'd1' }),
    codeBlock('ts', 'const f = (x: number) => x * x;\nf(2);', { id: 'k1', filename: 'square.ts' }),
    fileBlock('https://example.com/limits.pdf', 'limits.pdf', { pageCount: 12, sizeBytes: 482_000, id: 'f1' }),
  ],
});

const course = createDocument({
  title: 'Week 3: continuity',
  blocks: [
    paragraph('Watch the lecture before the exercise set; it unlocks once you have seen it through.', 'cp'),
    video(clip, {
      poster,
      durationMs: 10_000,
      caption: 'Lecture 3, recorded on 12 March.',
      chapters: [{ title: 'Definition', atMs: 0 }, { title: 'Three conditions', atMs: 3_000 }, { title: 'Worked examples', atMs: 6_000 }],
      id: 'v1',
    }),
    audio(clip, {
      title: 'Recap',
      durationMs: 10_000,
      transcript: [paragraph('A function is continuous at a point when its limit there equals its value.', 'tr1')],
      id: 'a1',
    }),
  ],
});

const handbook = createDocument({
  title: 'Course handbook',
  blocks: [
    paragraph('Everything you need for the term, in two files.', 'hp'),
    fileBlock('https://example.com/handbook.pdf', 'handbook.pdf', { pageCount: 36, sizeBytes: 2_400_000, display: 'inline', id: 'hf1' }),
    fileBlock('https://example.com/timetable.xlsx', 'timetable.xlsx', { sizeBytes: 64_000, description: 'Spring term', id: 'hf2' }),
  ],
});

/** A lesson: text, an image with its caption, a formula, a task list, a tip, a table, a quote, a fold, code and an attachment. */
export function Lesson() {
  return <JinshuView doc={lesson} data-testid="lesson" />;
}

/** A returning reader is offered their last position above the document. */
export function Resume() {
  return <JinshuView doc={lesson} resume={{ position: { blockId: 'h1', offset: 0 }, blocks: {}, media: {} }} data-testid="resume" />;
}

/** A course page whose video can only be scrubbed back over what was watched, at up to 1.5×. */
export function Course() {
  return <JinshuView doc={course} mediaPolicy={{ seek: 'played-only', maxRate: 1.5 }} data-testid="course" />;
}

/** Attachments: the app decides which files may show inline and handles the download. */
export function Attachments() {
  return (
    <Said>
      {say => (
        <JinshuView
          doc={handbook}
          filePolicy={{ default: 'attachment', inline: 'allow' }}
          onFileAction={({ src }) => say(`Downloading ${src.split('/').pop()}`)}
          data-testid="attachments"
        />
      )}
    </Said>
  );
}

/** No document: the reader says so in place of the content. */
export function Unavailable() {
  return <JinshuView doc={null} data-testid="unavailable" />;
}

/** Waiting for the document: placeholders where the title and the first blocks will be. */
export function Loading() {
  return <JinshuView doc={undefined} data-testid="loading" />;
}

/** The home tile. */
export function Specimen() {
  return (
    <JinshuView
      doc={createDocument({ title: 'Limits', blocks: [paragraph('Where a function heads.', 'sp'), callout('tip', [paragraph('Sketch it first.', 'sc')], { id: 'sk' })] })}
      isSelectable={false}
      className="w-40"
    />
  );
}
