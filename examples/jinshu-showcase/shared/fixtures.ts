import {
  audio,
  callout,
  codeBlock,
  columns,
  createDocument,
  createRegistry,
  customBlock,
  defineBlock,
  details,
  divider,
  fileBlock,
  heading,
  image,
  inlineMath,
  list,
  listItem,
  mathBlock,
  paragraph,
  quote,
  run,
  table,
  video,
  type JinshuDocument,
} from '@liulidesign/jinshu/schema';

export interface QuizAttrs extends Record<string, unknown> {
  readonly question: string;
  readonly options: readonly string[];
  readonly answerIndex: number;
  readonly explanation: string;
}

export const quizBlock = defineBlock<QuizAttrs>({
  type: 'x-demo-quiz',
  version: 1,
  attrsSchema: {
    type: 'object',
    required: ['question', 'options', 'answerIndex', 'explanation'],
    properties: {
      question: { type: 'string', minLength: 1 },
      options: { type: 'array', items: { type: 'string' } },
      answerIndex: { type: 'integer', minimum: 0 },
      explanation: { type: 'string' },
    },
  },
  content: 'none',
  aiDescription: 'Single-choice comprehension quiz card with instant feedback.',
  aiExample: {
    type: 'x-demo-quiz',
    attrs: {
      question: 'Why does every Jinshu block carry a stable id?',
      options: ['For CSS selectors', 'For lossless live reconcile & surgical AI patches'],
      answerIndex: 1,
      explanation: 'Stable IDs anchor AI edits, external links, observations, and live reconciliation.',
    },
  },
  fallbackText: attrs => `[测验] ${attrs.question}`,
});

export const demoRegistry = createRegistry({ blocks: [quizBlock] });

export const lessonDoc: JinshuDocument = createDocument({
  id: 'light-lesson',
  title: '光线入门：日出与黄昏怎么拍',
  lang: 'zh-CN',
  blocks: [
    paragraph(
      [
        run('同一片海，清晨和傍晚拍出来完全是两种颜色。这一课讲'),
        run('光的方向、色温和曝光', { bold: true }),
        run('，最后有一道'),
        run('小测验', { link: '#b_quiz1' }),
        run('检查你是否掌握了'),
        run('黄金时刻', { highlight: 'accent' }),
        run('的要点。'),
      ],
      'b_intro',
    ),
    audio('public/media/jinshu-intro.m4a', {
      id: 'b_audio1',
      title: '课程导读',
      durationMs: 8_400,
      chapters: [
        { atMs: 0, title: '这一课学什么' },
        { atMs: 3_200, title: '色温' },
        { atMs: 5_600, title: '练习' },
      ],
      transcript: [paragraph('这一课我们看两件事：光从哪个方向来，以及它是什么颜色。拍完之后，用课后练习检查自己的照片。', 'b_tr1')],
    }),
    heading(2, '一、光从哪里来', 'b_h_model'),
    paragraph(
      [
        run('太阳越低，光线越斜，影子越长，'),
        run('质感', { bold: true }),
        run('越明显。正午的顶光把一切压平，所以风光摄影师总在日出后和日落前的一小时出门。'),
      ],
      'b_p_model',
    ),
    columns(
      [
        [image('public/photos/photo-1.png', { id: 'b_img1', width: 480, height: 360, alt: '清晨的海面，太阳刚出地平线', caption: '日出：冷暖交界' })],
        [image('public/photos/photo-2.png', { id: 'b_img2', width: 480, height: 360, alt: '上午的山丘，阳光从侧面照来', caption: '上午：侧光的层次' })],
      ],
      { id: 'b_img_cols' },
    ),
    callout('tip', [paragraph('先看影子再按快门：影子朝向你，是逆光；朝向一侧，是侧光，最容易拍出立体感。', 'b_c_tip_p')], {
      id: 'b_c_tip',
      title: '一个习惯',
    }),
    heading(2, '二、色温与曝光', 'b_h_rich'),
    paragraph([run('色温用开尔文表示。数值越低越暖，越高越冷；相机的白平衡就是告诉它「什么是白」。')], 'b_p_temp'),
    table(
      ['时段', '色温（约）', '适合拍'],
      [
        ['日出后一小时', '3500 K', '海面、薄雾'],
        ['正午', '5500 K', '建筑、证件照'],
        ['阴天', '6500 K', '人像、花卉'],
        ['日落前一小时', '3000 K', '剪影、山丘'],
      ],
      { id: 'b_table1', align: ['start', 'end', 'start'] },
    ),
    paragraph([run('曝光值把光圈 '), inlineMath('N'), run(' 和快门 '), inlineMath('t'), run(' 合成一个数：')], 'b_p_ev'),
    mathBlock('EV = \\log_2 \\frac{N^2}{t}', 'b_math1'),
    paragraph([run('光圈 f/8、快门 1/125 秒时，'), inlineMath('EV \\approx 13'), run('，正是晴天的典型值。')], 'b_p_ev2'),
    video('public/media/lesson-clip.mp4', {
      id: 'b_video1',
      caption: '白平衡校准：用色卡找回真实的颜色',
      chapters: [
        { atMs: 0, title: '色卡' },
        { atMs: 2_000, title: '校准前后' },
      ],
    }),
    details(
      '进阶：为什么用开尔文表示颜色',
      [quote([paragraph('把一块黑体加热，它会先发红、再发白、最后发蓝。色温就是「和这种光颜色相同的黑体温度」。', 'b_q_p1')], 'b_quote1')],
      { id: 'b_details1' },
    ),
    heading(2, '三、出门前', 'b_h_media'),
    image('public/photos/photo-3.png', { id: 'b_img3', width: 480, height: 360, alt: '黄昏的山丘，天空泛紫', size: 'medium', caption: '黄昏：蓝调时刻的过渡' }),
    list(
      'task',
      [
        listItem('查好日出、日落时间', { id: 'b_li_1' }),
        listItem('白平衡设为「日光」，回来再调', { id: 'b_li_2' }),
        listItem('带一张灰卡', { id: 'b_li_3' }),
      ],
      { id: 'b_list_go' },
    ),
    fileBlock('./slides/shooting-checklist.pdf', '拍摄清单.pdf', {
      id: 'b_file1',
      display: 'inline',
      mimeType: 'application/pdf',
      sizeBytes: 2_450_000,
      pageCount: 4,
      description: '可打印，四页',
    }),
    divider('b_hr1'),
    heading(2, '四、课后小测', 'b_h_quiz'),
    customBlock(
      'x-demo-quiz',
      {
        question: '日落前一小时拍海面，照片整体偏橙。想保留这种暖意，白平衡应该怎么设？',
        options: ['自动白平衡', '日光（约 5500 K）', '钨丝灯（约 3200 K）'],
        answerIndex: 1,
        explanation: '日光白平衡按正午的光来还原，傍晚的暖光会如实留下来；自动和钨丝灯都会把橙色校正掉。',
      } satisfies QuizAttrs,
      '[测验] 傍晚的白平衡',
      { id: 'b_quiz1' },
    ),
  ],
});

export const draftDoc: JinshuDocument = createDocument({
  id: 'offline-reading-spec',
  title: '需求草稿：课程离线阅读',
  lang: 'zh-CN',
  blocks: [
    paragraph('学员在地铁和飞机上打不开课程，是上个月反馈最多的问题。这份草稿说明离线阅读要做到什么、先做哪些。', 'e_p1'),
    heading(2, '目标', 'e_h1'),
    list(
      'ordered',
      [
        listItem('已下载的课程在没有网络时能完整阅读，包括图片和音频', { id: 'e_li1' }),
        listItem('离线时记录的阅读进度，联网后合并，不丢不重', { id: 'e_li2' }),
        listItem('单门课程下载不超过 50 MB', { id: 'e_li3' }),
      ],
      { id: 'e_l1' },
    ),
    heading(2, '方案', 'e_h2'),
    paragraph('课程按块下载；视频只下载封面和字幕，正片仍需联网。缓存清单由服务端下发：', 'e_p2'),
    codeBlock(
      'json',
      ['{', '  "course": "light-101",', '  "version": 7,', '  "blocks": 42,', '  "assets": ["photo-1.png", "intro.m4a"],', '  "videoOffline": false', '}'].join('\n'),
      { id: 'e_code1', filename: 'manifest.json' },
    ),
    table(
      ['阶段', '内容', '时间'],
      [
        ['一', '图文与音频离线', '12 月'],
        ['二', '离线进度合并', '1 月'],
        ['三', '视频离线（视版权）', '待定'],
      ],
      { id: 'e_table1' },
    ),
    callout('warning', [paragraph('视频离线涉及版权方授权，第三阶段之前需要法务确认。', 'e_c1_p')], { id: 'e_c1', title: '待确认' }),
    heading(2, '开放问题', 'e_h3'),
    list(
      'task',
      [
        listItem('课程更新后，已下载的旧版本保留多久？', { id: 'e_q1' }),
        listItem('存储空间不足时，先清理哪些课程？', { id: 'e_q2' }),
      ],
      { id: 'e_l2' },
    ),
  ],
});

export const meetingDoc: JinshuDocument = createDocument({
  id: 'weekly-notes',
  title: '周会纪要 · 11 月 24 日',
  lang: 'zh-CN',
  blocks: [
    paragraph([run('参会：', { bold: true }), run('产品、设计、客户端、服务端各一人。主持：林舟。')], 'm_p_who'),
    heading(2, '本周结论', 'm_h1'),
    list(
      'bullet',
      [
        listItem('离线阅读第一阶段只做图文与音频，视频等版权确认', { id: 'm_li1' }),
        listItem('课程下载入口放在课程详情页，不放首页', { id: 'm_li2' }),
        listItem('下载中断后从断点继续，不重新下载', { id: 'm_li3' }),
      ],
      { id: 'm_l1' },
    ),
    heading(2, '待办', 'm_h2'),
    list(
      'task',
      [
        listItem('出下载入口的设计稿', { id: 'm_t1', checked: true }),
        listItem('确定缓存清单的字段', { id: 'm_t2' }),
        listItem('联系版权方，确认视频离线条款', { id: 'm_t3' }),
      ],
      { id: 'm_l2' },
    ),
    table(
      ['事项', '负责人', '截止'],
      [
        ['下载入口设计稿', '设计', '11 月 28 日'],
        ['缓存清单字段', '服务端', '11 月 27 日'],
        ['视频版权确认', '产品', '12 月 2 日'],
      ],
      { id: 'm_tbl' },
    ),
    callout('warning', [paragraph('版权结论出来之前，任何宣传材料都不要提「视频离线」。', 'm_c_p')], {
      id: 'm_c1',
      title: '风险',
    }),
    paragraph('下次周会：12 月 1 日。', 'm_p_next'),
  ],
});

const LONG_TOPICS = ['器材', '构图', '光线', '色彩', '人像', '风光', '街头', '后期'];

/** A long document for scrolling, finding and an agent's outline reads: 40 sections of 20 blocks. */
export function longDoc(): JinshuDocument {
  const blocks = [];
  for (let s = 1; s <= 40; s++) {
    const topic = LONG_TOPICS[(s - 1) % LONG_TOPICS.length];
    const round = Math.ceil(s / LONG_TOPICS.length);
    blocks.push(heading(2, `${s}. ${topic} · 第 ${round} 讲`, `long_h${s}`));
    for (let b = 1; b <= 19; b++) {
      const id = `long_${s}_${b}`;
      if (b === 6) {
        blocks.push(list('bullet', [listItem(`${topic}：先观察，再动手`, { id: `${id}_a` }), listItem(`${topic}：每次只改一个变量`, { id: `${id}_b` })], { id }));
      } else if (b === 12) {
        blocks.push(callout('tip', [paragraph(`练习：用同一场景拍三张，只改变${topic}，对比差别。`, `${id}_p`)], { id }));
      } else if (b === 17) {
        blocks.push(quote([paragraph(`第 ${s} 讲小结：${topic}没有标准答案，多拍、多看、多比较。`, `${id}_p`)], id));
      } else {
        blocks.push(
          paragraph(
            `第 ${s} 讲第 ${b} 段。关于${topic}，最常见的问题是想得太多、拍得太少。带着一个具体的问题出门，回来对照照片检查答案，比读十篇文章更有用。`,
            id,
          ),
        );
      }
    }
  }
  return createDocument({ id: 'photo-handbook', title: '摄影手册（800 块）', lang: 'zh-CN', blocks });
}
