//! What the text stack does for a run that must keep its own direction inside a line of the
//! other: visual order without and with an isolate (FSI…PDI), the isolate's width, and the
//! shaping cost. A measurement; run with `--ignored --nocapture`.
use parley::{BaseDirection, GenericFamily, LayoutContext, PositionedLayoutItem};

const FSI: char = '\u{2068}';
const PDI: char = '\u{2069}';

/// The line's characters left to right, and the summed advance of isolate marks.
fn visual(text: &str, base: BaseDirection) -> (String, f32) {
    let mut fonts = crate::text::fonts::context();
    let mut layouts = LayoutContext::<()>::new();
    let mut builder = layouts.ranged_builder(&mut fonts, text, 1.0, true);
    builder.set_base_direction(base);
    builder.push_default(GenericFamily::SansSerif);
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    let (mut out, mut marks) = (String::new(), 0.0);
    for line in layout.lines() {
        let mut runs: Vec<_> = line
            .items()
            .filter_map(|item| match item {
                PositionedLayoutItem::GlyphRun(run) => Some(run),
                _ => None,
            })
            .collect();
        runs.sort_by(|a, b| a.offset().total_cmp(&b.offset()));
        for run in runs {
            for cluster in run.run().visual_clusters() {
                let piece = &text[cluster.text_range()];
                if piece.contains([FSI, PDI]) {
                    marks += cluster.advance();
                }
                out.push_str(&piece.replace([FSI, PDI], ""));
            }
        }
    }
    (out, marks)
}

fn isolate(text: &str, part: &str) -> String {
    text.replacen(part, &format!("{FSI}{part}{PDI}"), 1)
}

#[test]
#[ignore = "measurement"]
fn bidi_isolation_measurement() {
    let cases = [
        ("זמן: 2 h", "2 h", BaseDirection::Rtl),
        ("מהירות 487 Mbps היום", "487 Mbps", BaseDirection::Rtl),
        ("السرعة 487 Mbps اليوم", "487 Mbps", BaseDirection::Rtl),
        ("المدة: 2 h", "2 h", BaseDirection::Rtl),
        ("Owner: שרה כהן 3", "שרה כהן", BaseDirection::Ltr),
        ("Sender: أحمد 12 messages", "أحمد", BaseDirection::Ltr),
    ];
    for (text, part, base) in cases {
        let (plain, _) = visual(text, base);
        let (isolated, marks) = visual(&isolate(text, part), base);
        println!("{base:?} {text:?} part {part:?}\n  plain    {plain:?}\n  isolated {isolated:?}  marks {marks}px");
    }
    let lines: Vec<String> = (0..2000)
        .map(|i| format!("מהירות {i} Mbps, זמן {} h, שם Example {i}", i % 24))
        .collect();
    let isolated: Vec<String> = lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            isolate(
                &isolate(&isolate(line, &format!("{i} Mbps")), &format!("{} h", i % 24)),
                &format!("Example {i}"),
            )
        })
        .collect();
    let time = |lines: &[String]| {
        let mut fonts = crate::text::fonts::context();
        let mut layouts = LayoutContext::<()>::new();
        let started = std::time::Instant::now();
        for line in lines {
            let mut builder = layouts.ranged_builder(&mut fonts, line, 1.0, true);
            builder.set_base_direction(BaseDirection::Rtl);
            builder.push_default(GenericFamily::SansSerif);
            let mut layout = builder.build(line);
            layout.break_all_lines(Some(240.0));
        }
        started.elapsed()
    };
    time(&lines);
    let (plain, with) = (time(&lines), time(&isolated));
    println!(
        "2000 RTL lines, 3 isolates each: plain {plain:?}, isolated {with:?} ({:+.1}%)",
        (with.as_secs_f64() / plain.as_secs_f64() - 1.0) * 100.0
    );
}
