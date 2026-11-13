//! Composite widgets: a container whose items share one Tab stop, with arrow keys moving focus
//! between them (roving focus). Membership comes from the laid-out tree, since a kit cannot list
//! the items it composes.
use super::frame::{PresentedFrame, clickable, hit_target};
use crate::scene::node::NodeSnapshot;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Composite {
    Menu,
    TabList,
    RadioGroup,
    Toolbar,
    ListBox,
    Tree,
}

/// The direction a key moves focus in, or the end it jumps to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Next,
    Previous,
    First,
    Last,
    /// A tree's Right: the first child of an expanded item.
    Child,
    /// A tree's Left: the item one level up.
    Parent,
}

impl Composite {
    pub fn of(node: &NodeSnapshot) -> Option<Self> {
        Some(match node.role.as_deref()? {
            "menu" => Self::Menu,
            "tablist" => Self::TabList,
            "radiogroup" => Self::RadioGroup,
            "toolbar" => Self::Toolbar,
            "listbox" => Self::ListBox,
            "tree" => Self::Tree,
            _ => return None,
        })
    }

    fn item(self, node: &NodeSnapshot) -> bool {
        let role = node.role.as_deref();
        match self {
            Self::Menu => matches!(
                role,
                Some("menuitem" | "menuitemcheckbox" | "menuitemradio")
            ),
            Self::TabList => role == Some("tab"),
            Self::RadioGroup => role == Some("radio"),
            Self::ListBox => role == Some("option"),
            Self::Tree => role == Some("treeitem"),
            // Any control, except one whose arrows already mean something of its own.
            Self::Toolbar => {
                hit_target(node)
                    && node.range.is_none()
                    && node.select.is_none()
                    && node.drag.is_none()
                    && node.tag != "radio-option"
            }
        }
    }

    /// Tabs and radios activate when focus arrives, as ARIA's automatic activation does.
    pub fn activates(self) -> bool {
        matches!(self, Self::TabList | Self::RadioGroup)
    }

    fn wraps(self) -> bool {
        !matches!(self, Self::ListBox | Self::Tree)
    }

    /// The state that makes an item the group's Tab stop.
    fn chosen(self, node: &NodeSnapshot) -> bool {
        match self {
            Self::RadioGroup => node.states[6] == Some(true),
            Self::TabList | Self::ListBox | Self::Tree => node.states[1] == Some(true),
            Self::Menu | Self::Toolbar => false,
        }
    }

    /// What `key` does in this composite; `rtl` swaps Left and Right. `None` leaves the key alone.
    pub fn movement(self, key: super::view::Key, rtl: bool) -> Option<Move> {
        use super::view::Key;
        let (left, right) = if rtl {
            (Move::Next, Move::Previous)
        } else {
            (Move::Previous, Move::Next)
        };
        Some(match (self, key) {
            (_, Key::Home) if self != Self::RadioGroup => Move::First,
            (_, Key::End) if self != Self::RadioGroup => Move::Last,
            (Self::Menu | Self::ListBox | Self::Tree, Key::Up) => Move::Previous,
            (Self::Menu | Self::ListBox | Self::Tree, Key::Down) => Move::Next,
            (Self::TabList | Self::Toolbar | Self::RadioGroup, Key::Left) => left,
            (Self::TabList | Self::Toolbar | Self::RadioGroup, Key::Right) => right,
            (Self::RadioGroup, Key::Up) => Move::Previous,
            (Self::RadioGroup, Key::Down) => Move::Next,
            (Self::Tree, Key::Right) => {
                if rtl {
                    Move::Parent
                } else {
                    Move::Child
                }
            }
            (Self::Tree, Key::Left) => {
                if rtl {
                    Move::Child
                } else {
                    Move::Parent
                }
            }
            _ => return None,
        })
    }

    /// Menus, list boxes and trees jump to the next item whose name starts with a typed character.
    pub fn types_ahead(self) -> bool {
        matches!(self, Self::Menu | Self::ListBox | Self::Tree)
    }
}

fn descends(node: &NodeSnapshot, ancestor: &NodeSnapshot) -> bool {
    node.identity.len() > ancestor.identity.len()
        && node.identity.starts_with(&*ancestor.identity)
        && node.identity.as_bytes()[ancestor.identity.len()] == b'/'
}

/// Every node's composite, worked out once per use.
pub struct Composites {
    containers: Vec<(usize, Composite)>,
    owner: Vec<Option<(usize, Composite)>>,
}

impl Composites {
    pub fn new(frame: &PresentedFrame) -> Self {
        let containers: Vec<(usize, Composite)> = frame
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| Composite::of(node).map(|kind| (index, kind)))
            .collect();
        let owner = frame
            .nodes
            .iter()
            .map(|node| {
                if containers.is_empty() {
                    return None;
                }
                // The nearest composite ancestor owns the node, if the node is its kind of item.
                let (container, kind) = containers
                    .iter()
                    .filter(|(container, _)| descends(node, &frame.nodes[*container]))
                    .max_by_key(|(container, _)| frame.nodes[*container].identity.len())?;
                kind.item(node).then_some((*container, *kind))
            })
            .collect();
        Self { containers, owner }
    }

    pub fn container_of(&self, index: usize) -> Option<(usize, Composite)> {
        self.owner.get(index).copied().flatten()
    }

    pub fn is_empty(&self) -> bool {
        self.containers.is_empty()
    }

    /// `container`'s focusable items in tree order.
    pub fn items(
        &self,
        frame: &PresentedFrame,
        container: usize,
        revealable: impl Fn(&NodeSnapshot) -> bool,
    ) -> Vec<usize> {
        (0..frame.nodes.len())
            .filter(|index| {
                self.container_of(*index)
                    .is_some_and(|(owner, _)| owner == container)
                    && clickable(&frame.nodes[*index])
                    && super::keyboard::candidate(frame, *index, &revealable)
            })
            .collect()
    }

    /// The item of `container` that takes its Tab stop: the chosen one, else the one focused last,
    /// else the first. Menus keep every item a stop.
    pub fn tab_stop(
        &self,
        frame: &PresentedFrame,
        index: usize,
        remembered: impl Fn(usize) -> Option<usize>,
        revealable: impl Fn(&NodeSnapshot) -> bool,
    ) -> bool {
        let Some((container, kind)) = self.container_of(index) else {
            return true;
        };
        if kind == Composite::Menu {
            return true;
        }
        let items = self.items(frame, container, revealable);
        let stop = items
            .iter()
            .copied()
            .find(|item| kind.chosen(&frame.nodes[*item]))
            .or_else(|| remembered(container).filter(|item| items.contains(item)))
            .or_else(|| items.first().copied());
        stop == Some(index)
    }

    /// The item a move from `index` lands on; `None` when there is none to move to.
    pub fn step(
        &self,
        frame: &PresentedFrame,
        index: usize,
        movement: Move,
        revealable: impl Fn(&NodeSnapshot) -> bool,
    ) -> Option<usize> {
        let (container, kind) = self.container_of(index)?;
        let items = self.items(frame, container, revealable);
        let at = items.iter().position(|item| *item == index)?;
        let count = items.len();
        let level = |item: usize| frame.nodes[item].heading_level.unwrap_or(1);
        match movement {
            Move::First => items.first().copied(),
            Move::Last => items.last().copied(),
            Move::Next if at + 1 < count => Some(items[at + 1]),
            Move::Next => kind.wraps().then(|| items[0]),
            Move::Previous if at > 0 => Some(items[at - 1]),
            Move::Previous => kind.wraps().then(|| items[count - 1]),
            Move::Child => items
                .get(at + 1)
                .copied()
                .filter(|_| frame.nodes[index].states[2] == Some(true))
                .filter(|next| level(*next) > level(index)),
            Move::Parent => items[..at]
                .iter()
                .rev()
                .copied()
                .find(|item| level(*item) < level(index)),
        }
    }
}
