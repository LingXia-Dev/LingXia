//! The floating text menu: whether it shows, where, and with which actions.
//! Adapters draw it and report clicks and user dismissal; core decides the rest.
use super::Inner;
use crate::platform::PlatformAdapter;
use crate::scene::node::NodeSnapshot;
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuAction {
    Cut,
    Copy,
    /// Always offered in an editor; the platform reads its clipboard only on click.
    Paste,
    SelectAll,
    Undo,
    Redo,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MenuTarget {
    /// Selectable static text; core runs its actions.
    Text { identity: String },
    /// The focused editor; the platform runs its actions through the input connection.
    Editor { identity: String },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TextMenu {
    pub target: MenuTarget,
    /// Logical rect the menu points at.
    pub anchor: [f32; 4],
    pub actions: Vec<MenuAction>,
}

/// An editor menu asked for by a word selection or a handle release.
#[derive(Debug)]
struct EditorRequest {
    identity: String,
    /// Text and whether a range was selected when it first showed; either
    /// changing dismisses it.
    shown: Option<(String, bool)>,
}

#[derive(Debug, Default)]
pub(super) struct MenuState {
    editor: Option<EditorRequest>,
    dragging: bool,
    shown: Option<TextMenu>,
}

fn union(rects: &[[f32; 4]]) -> Option<[f32; 4]> {
    let mut rects = rects.iter();
    let first = *rects.next()?;
    Some(rects.fold(first, |[x, y, w, h], r| {
        let (left, top) = (x.min(r[0]), y.min(r[1]));
        let (right, bottom) = ((x + w).max(r[0] + r[2]), (y + h).max(r[1] + r[3]));
        [left, top, right - left, bottom - top]
    }))
}

impl<F> super::Main<F> {
    pub(super) fn text_menu_shown(&self) -> Option<TextMenu> {
        self.text_menu.shown.clone()
    }
}

impl MenuState {
    fn decide(&mut self, nodes: &[NodeSnapshot]) -> Option<TextMenu> {
        if self.dragging {
            return None;
        }
        if let Some(node) = nodes
            .iter()
            .find(|node| node.input.is_none() && node.text_selection.is_some())
        {
            let selection = node.text_selection.as_ref().unwrap();
            let mut actions = Vec::new();
            if node.selection_range.is_some_and(|[a, b]| a != b) {
                actions.push(MenuAction::Copy);
            }
            actions.push(MenuAction::SelectAll);
            return Some(TextMenu {
                target: MenuTarget::Text {
                    identity: node.identity.clone(),
                },
                anchor: union(&selection.selection).unwrap_or(node.bounds),
                actions,
            });
        }
        let request = self.editor.as_mut()?;
        let found = nodes.iter().find_map(|node| {
            let input = node.input.as_ref()?;
            let edit = input.edit.as_ref()?;
            (node.identity == request.identity && input.edit_token.is_some_and(|t| t > 0))
                .then_some((node, input, edit))
        });
        let Some((node, input, edit)) = found else {
            self.editor = None;
            return None;
        };
        let selected = edit.selection[0] != edit.selection[1];
        match &request.shown {
            None => request.shown = Some((edit.text.clone(), selected)),
            Some((text, was_selected)) if *text != edit.text || (*was_selected && !selected) => {
                self.editor = None;
                return None;
            }
            Some(_) => {}
        }
        let mut actions = Vec::new();
        if selected && !input.secure {
            actions.extend([MenuAction::Cut, MenuAction::Copy]);
        }
        actions.push(MenuAction::Paste);
        if !edit.text.is_empty() {
            actions.push(MenuAction::SelectAll);
        }
        if edit.can_undo {
            actions.push(MenuAction::Undo);
        }
        if edit.can_redo {
            actions.push(MenuAction::Redo);
        }
        Some(TextMenu {
            target: MenuTarget::Editor {
                identity: node.identity.clone(),
            },
            anchor: node.bounds,
            actions,
        })
    }
}

impl<A: PlatformAdapter> Inner<A> {
    /// Re-decide against the presented frame and tell the adapter about a change.
    pub(super) fn update_text_menu(self: &Arc<Self>) {
        let changed = {
            let mut main = self.main();
            let snapshot = main.snapshot.clone();
            let next = snapshot.and_then(|s| main.text_menu.decide(&s.frame.nodes));
            (main.text_menu.shown != next).then(|| {
                main.text_menu.shown = next.clone();
                next
            })
        };
        if let Some(menu) = changed {
            self.adapter.text_menu(menu.as_ref());
        }
    }

    /// A long-press word selection or released handle asks for the editor's menu.
    pub(super) fn request_editor_menu(self: &Arc<Self>, identity: String) {
        self.main().text_menu.editor = Some(EditorRequest {
            identity,
            shown: None,
        });
        self.update_text_menu();
    }

    /// A touch on the page or a stopped caret dismisses the editor menu.
    pub(super) fn dismiss_editor_menu(self: &Arc<Self>) {
        let had = self.main().text_menu.editor.take().is_some();
        if had {
            self.update_text_menu();
        }
    }

    pub(super) fn set_handle_drag(self: &Arc<Self>, active: bool) {
        let editor = {
            let mut main = self.main();
            main.text_menu.dragging = active;
            // Releasing an editor handle shows its menu again.
            (!active)
                .then(|| {
                    main.snapshot.as_ref().and_then(|s| {
                        s.frame
                            .nodes
                            .iter()
                            .find(|n| {
                                n.input
                                    .as_ref()
                                    .is_some_and(|i| i.edit_token.is_some_and(|t| t > 0))
                            })
                            .map(|n| n.identity.clone())
                    })
                })
                .flatten()
        };
        match editor {
            Some(identity) => self.request_editor_menu(identity),
            None => self.update_text_menu(),
        }
    }

    /// The user closed the menu (back, outside touch): a static selection goes with it.
    pub(super) fn text_menu_dismissed(self: &Arc<Self>) -> bool {
        let shown = {
            let mut main = self.main();
            main.text_menu.editor = None;
            main.text_menu.shown.take()
        };
        matches!(
            shown,
            Some(TextMenu {
                target: MenuTarget::Text { .. },
                ..
            })
        )
    }
}
