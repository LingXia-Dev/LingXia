use crate::protocol::field;
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
        timeline: Box<Timeline>,
        now: Duration,
        /// When the frame is expected on screen.
        sample: Duration,
        /// The unpresented frame this one is prepared ahead of.
        base: Option<Box<Prepared>>,
        frame: Option<Box<Prepared>>,
        visible: BTreeSet<String>,
    },
}

impl FrameAnimations<'_> {
    pub(super) fn validate(&self, records: &[crate::protocol::Record]) -> Result<(), String> {
        match self {
            Self::Disabled => validate_record_animations(records, None),
            Self::Explicit(frame) => validate_record_animations(records, Some(frame)),
            Self::Collected { .. } => Ok(()),
        }
    }

    pub(super) fn compose(
        &mut self,
        records: &[crate::protocol::Record],
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
                sample,
                base,
                frame,
                visible,
            } => {
                let candidate = inventory.reconcile(records)?;
                let transitions = transition_inputs(records, &display)?;
                let realized: BTreeSet<_> = records
                    .iter()
                    .zip(&display.nodes)
                    .filter(|(record, _)| {
                        record.string(field::ANIMATION).is_some()
                            || record.string(field::TRANSITION).is_some()
                    })
                    .map(|(_, node)| node.identity.clone())
                    .collect();
                let preview = timeline.prepare_frame(
                    *now,
                    *sample,
                    candidate.definitions(),
                    transitions.clone(),
                    realized.iter().map(String::as_str),
                    base.as_deref(),
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
                    timeline.discard(&preview);
                    let prepared = timeline.prepare_frame(
                        *now,
                        *sample,
                        candidate.definitions(),
                        transitions,
                        visible.iter().map(String::as_str),
                        base.as_deref(),
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

/// Each realized node with a transition, paired with its natively resolved paint.
fn transition_inputs(
    records: &[crate::protocol::Record],
    display: &DisplayList,
) -> Result<
    Vec<(
        String,
        crate::animation::transition::TransitionDef,
        crate::animation::transition::Resolved,
    )>,
    String,
> {
    let mut blocked = None;
    let mut inputs = Vec::new();
    for (index, record) in records.iter().enumerate() {
        let Some(source) = record.string(field::TRANSITION) else {
            continue;
        };
        let retained = &display.retained;
        let blocked = blocked.get_or_insert_with(|| retained.transform_blocked());
        inputs.push((
            retained.nodes[index].identity.clone(),
            crate::animation::transition::TransitionDef::decode(source)?,
            retained.resolved(index, blocked[index]),
        ));
    }
    Ok(inputs)
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

fn validate_record_animations(
    records: &[crate::protocol::Record],
    frame: Option<&crate::animation::Prepared>,
) -> Result<(), String> {
    if !records
        .iter()
        .any(|record| record.string(field::ANIMATION).is_some())
    {
        return Ok(());
    }
    let frame = frame.ok_or("UI_ANIMATION: page requires animation presentation scheduling")?;
    for (record, identity) in records.iter().zip(crate::protocol::identities(records)) {
        if let Some(source) = record.string(field::ANIMATION) {
            let definition = crate::animation::decode_definition(source)?;
            if !frame.contains_definition(&identity, &definition) {
                return Err("UI_ANIMATION: frame definition does not match View record".into());
            }
        }
    }
    Ok(())
}
