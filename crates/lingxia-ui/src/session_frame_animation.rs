use crate::{
    DisplayList,
    animation::{Prepared, Timeline, records::Inventory},
};
use std::{
    collections::{BTreeSet, HashMap},
    time::Duration,
};

pub(super) enum FrameAnimations<'a> {
    Disabled,
    Explicit(&'a Prepared),
    Collected {
        inventory: Inventory,
        timeline: Timeline,
        now: Duration,
        frame: Option<Box<Prepared>>,
        visible: BTreeSet<String>,
    },
}

impl FrameAnimations<'_> {
    pub(super) fn validate(&self, records: &[crate::schema::Record]) -> Result<(), String> {
        match self {
            Self::Disabled => super::validate_record_animations(records, None),
            Self::Explicit(frame) => super::validate_record_animations(records, Some(frame)),
            Self::Collected { .. } => Ok(()),
        }
    }

    pub(super) fn compose(
        &mut self,
        records: &[crate::schema::Record],
        display: DisplayList,
        offsets: &HashMap<String, f32>,
    ) -> Result<(DisplayList, bool), String> {
        match self {
            Self::Disabled => Ok((display, false)),
            Self::Explicit(frame) => compose(display, offsets, frame)
                .map(|display| (display, !frame.visuals().is_empty())),
            Self::Collected {
                inventory,
                timeline,
                now,
                frame,
                visible,
            } => {
                let candidate = inventory.reconcile(records)?;
                let realized: BTreeSet<_> = records
                    .iter()
                    .zip(&display.nodes)
                    .filter(|(record, _)| record.string(101).is_some())
                    .map(|(_, node)| node.identity.clone())
                    .collect();
                let preview = timeline.prepare_visible(
                    *now,
                    candidate.definitions(),
                    realized.iter().map(String::as_str),
                )?;
                let mut recomposed = !preview.visuals().is_empty();
                let base_visible = visible_nodes(&display, &realized);
                let rendered = compose(display, offsets, &preview)?;
                *visible = base_visible
                    .union(&visible_nodes(&rendered, &realized))
                    .cloned()
                    .collect();
                let (prepared, rendered) = if *visible == realized {
                    (preview, rendered)
                } else {
                    recomposed = true;
                    let prepared = timeline.prepare_visible(
                        *now,
                        candidate.definitions(),
                        visible.iter().map(String::as_str),
                    )?;
                    let rendered = rendered
                        .retained
                        .compose_animated(offsets, prepared.visuals())?;
                    (prepared, rendered)
                };
                *inventory = candidate;
                *frame = Some(Box::new(prepared));
                Ok((rendered, recomposed))
            }
        }
    }
}

fn visible_nodes(display: &DisplayList, realized: &BTreeSet<String>) -> BTreeSet<String> {
    display
        .nodes
        .iter()
        .filter(|node| {
            realized.contains(&node.identity)
                && node.visible_bounds[2] > 0.0
                && node.visible_bounds[3] > 0.0
        })
        .map(|node| node.identity.clone())
        .collect()
}

fn compose(
    display: DisplayList,
    offsets: &HashMap<String, f32>,
    frame: &Prepared,
) -> Result<DisplayList, String> {
    if frame.visuals().is_empty() {
        Ok(display)
    } else {
        display.retained.compose_animated(offsets, frame.visuals())
    }
}
