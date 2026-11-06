//! The layout tree kept across frames: nodes keyed by record identity, so Taffy's own cache
//! survives a render and only nodes whose style, text or children changed lay out again.
use std::collections::{HashMap, HashSet};
use taffy::prelude::*;

/// What every node's layout depends on besides its own style and text.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct FrameKey {
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) scale: u32,
    pub(crate) baselines: bool,
}

pub(crate) struct LayoutTree {
    pub(crate) tree: TaffyTree<usize>,
    key: FrameKey,
    nodes: HashMap<String, (NodeId, Option<u64>)>,
    /// A list's leading and trailing spacers, by the list's identity.
    spacers: HashMap<String, [NodeId; 2]>,
    seen: HashSet<NodeId>,
}

impl LayoutTree {
    /// The tree for a frame with `key`: the kept one when the frame matches, else a new one.
    pub(crate) fn reuse(kept: Option<Self>, key: FrameKey) -> Self {
        match kept {
            Some(mut kept) if kept.key == key => {
                kept.seen.clear();
                kept
            }
            _ => Self {
                tree: TaffyTree::new(),
                key,
                nodes: HashMap::new(),
                spacers: HashMap::new(),
                seen: HashSet::new(),
            },
        }
    }

    /// The node for record `index` at `identity`. `measure` names what its measured size
    /// depends on beyond its style; `None` measures it again every frame.
    pub(crate) fn node(
        &mut self,
        identity: &str,
        style: Style,
        index: usize,
        measure: Option<u64>,
    ) -> Result<NodeId, String> {
        let id = match self.nodes.get_mut(identity) {
            Some((id, kept)) => {
                let id = *id;
                if self.tree.style(id).map_err(|e| e.to_string())? != &style {
                    self.tree.set_style(id, style).map_err(|e| e.to_string())?;
                }
                if measure.is_none() || *kept != measure {
                    self.tree.mark_dirty(id).map_err(|e| e.to_string())?;
                }
                *kept = measure;
                // The record index moves between renders; it is not an input to layout.
                if let Some(context) = self.tree.get_node_context_mut(id) {
                    *context = index;
                }
                id
            }
            None => {
                let id = self
                    .tree
                    .new_leaf_with_context(style, index)
                    .map_err(|e| e.to_string())?;
                self.nodes.insert(identity.to_owned(), (id, measure));
                id
            }
        };
        self.seen.insert(id);
        Ok(id)
    }

    /// The leading (0) or trailing (1) spacer of the list at `identity`.
    pub(crate) fn spacer(
        &mut self,
        identity: &str,
        edge: usize,
        height: f32,
    ) -> Result<NodeId, String> {
        // Zero-width spacers do not contribute to Taffy's scroll overflow.
        let style = Style {
            size: Size {
                width: Dimension::AUTO,
                height: length(height),
            },
            flex_shrink: 0.0,
            ..Default::default()
        };
        let id = match self.spacers.get(identity) {
            Some(pair) => {
                let id = pair[edge];
                if self.tree.style(id).map_err(|e| e.to_string())? != &style {
                    self.tree.set_style(id, style).map_err(|e| e.to_string())?;
                }
                id
            }
            None => {
                let lead = self
                    .tree
                    .new_leaf(style.clone())
                    .map_err(|e| e.to_string())?;
                let trail = self.tree.new_leaf(style).map_err(|e| e.to_string())?;
                self.spacers.insert(identity.to_owned(), [lead, trail]);
                [lead, trail][edge]
            }
        };
        self.seen.insert(id);
        Ok(id)
    }

    pub(crate) fn set_children(
        &mut self,
        parent: NodeId,
        children: &[NodeId],
    ) -> Result<(), String> {
        if self.tree.children(parent).map_err(|e| e.to_string())? != children {
            self.tree
                .set_children(parent, children)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Drops the nodes this frame did not use.
    pub(crate) fn prune(&mut self) -> Result<(), String> {
        let Self {
            tree,
            nodes,
            spacers,
            seen,
            ..
        } = self;
        let mut gone = Vec::new();
        nodes.retain(|_, (id, _)| {
            seen.contains(id) || {
                gone.push(*id);
                false
            }
        });
        spacers.retain(|_, pair| {
            seen.contains(&pair[0]) || {
                gone.extend(*pair);
                false
            }
        });
        for id in gone {
            tree.remove(id).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
