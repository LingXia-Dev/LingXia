use crate::protocol::field;
use crate::{
    DisplayList,
    animation::{Prepared, Timeline, records::Inventory},
};
use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
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
        visible: BTreeSet<crate::scene::node::Identity>,
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
        palette: &crate::theme::Palette,
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
                let transitions = transition_inputs(records, &display, palette)?;
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
                    realized.iter().map(|id| &**id),
                    base.as_deref(),
                )?;
                let mut recomposed = !preview.visuals().is_empty();
                let base_visible = visible_nodes(&display, &realized);
                let rendered = compose(display, offsets, &preview)?;
                *visible = base_visible
                    .union(&visible_nodes(&rendered, &realized))
                    .cloned()
                    .collect();
                // Visibility gates keyframe animations only; transitions sample regardless.
                let (prepared, rendered) = if realized
                    .difference(visible)
                    .all(|identity| !candidate.contains(identity))
                {
                    (preview, rendered)
                } else {
                    recomposed = true;
                    timeline.discard(&preview);
                    let prepared = timeline.prepare_frame(
                        *now,
                        *sample,
                        candidate.definitions(),
                        transitions,
                        visible.iter().map(|id| &**id),
                        base.as_deref(),
                    )?;
                    // Dropping off-screen leaves changes no pixel and moves no other node.
                    if dropped_leaves(&rendered, &realized, visible) {
                        (prepared, rendered)
                    } else {
                        let rendered = compose(rendered, offsets, &prepared)?;
                        (prepared, rendered)
                    }
                };
                *inventory = candidate;
                *frame = Some(Box::new(prepared));
                Ok((rendered, recomposed))
            }
        }
    }
}

/// Each realized node with a transition: its natively resolved paint, the presence values it
/// enters from and leaves toward, and where it was laid out (for an exit ghost).
fn transition_inputs(
    records: &[crate::protocol::Record],
    display: &DisplayList,
    palette: &crate::theme::Palette,
) -> Result<Vec<crate::animation::transition::Input>, String> {
    use crate::animation::{presence, transition};
    let mut blocked = None;
    let mut inputs = Vec::new();
    for (index, record) in records.iter().enumerate() {
        let Some(source) = record.string(field::TRANSITION) else {
            continue;
        };
        let retained = &display.retained;
        let blocked = blocked.get_or_insert_with(|| retained.transform_blocked());
        let target = retained.resolved(index, blocked[index]);
        let [_, _, w, h] = retained.nodes[index].bounds;
        let presence = |id| {
            record
                .string(id)
                .map(|delta| presence::apply(&target, delta, palette, [w, h]))
                .transpose()
        };
        let (starting, exit) = (
            presence(field::STARTING_STYLE)?,
            presence(field::EXIT_STYLE)?,
        );
        inputs.push(transition::Input {
            identity: retained.nodes[index].identity.to_string().into(),
            definition: transition::TransitionDef::decode(source)?,
            origin: exit
                .is_some()
                .then(|| transition::Origin(Arc::clone(retained), index)),
            target,
            starting,
            exit,
        });
    }
    Ok(inputs)
}

/// Every animated node left out of `visible` is a leaf of `display`.
fn dropped_leaves(
    display: &DisplayList,
    realized: &BTreeSet<crate::scene::node::Identity>,
    visible: &BTreeSet<crate::scene::node::Identity>,
) -> bool {
    let retained = &display.retained;
    let mut parents = vec![false; retained.nodes.len()];
    for parent in retained.parents.iter().flatten() {
        if let Some(seen) = parents.get_mut(*parent) {
            *seen = true;
        }
    }
    retained.nodes.iter().zip(parents).all(|(node, parent)| {
        !parent || !realized.contains(&*node.identity) || visible.contains(&*node.identity)
    })
}

fn visible_nodes(
    display: &DisplayList,
    realized: &BTreeSet<crate::scene::node::Identity>,
) -> BTreeSet<crate::scene::node::Identity> {
    display
        .nodes
        .iter()
        .filter(|node| {
            realized.contains(&*node.identity)
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
    let ghosts = frame.ghosts();
    if frame.visuals().is_empty() && ghosts.is_empty() {
        Ok(display)
    } else if ghosts.is_empty() {
        display.retained.compose_animated(offsets, frame.visuals())
    } else {
        display
            .retained
            .with_ghosts(&ghosts)
            .compose_animated(offsets, frame.visuals())
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
