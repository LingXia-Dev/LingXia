use crate::{ShellError, ShellResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// The header strip is shared with the window controls, so it holds a few
/// actions rather than an open-ended list. Exceeding it rejects the whole
/// declaration instead of truncating: silently dropping one entry would leave
/// an app believing it published something the user cannot see.
///
/// The header is a borrowed corner of the window's caption row, shared with the
/// traffic lights or app menu and the collapse toggle. Two icon-only buttons is
/// what reads as a pair a person can learn; past that it becomes a row of
/// unlabelled glyphs in the part of the window that belongs to the window.
///
/// Exceeding it rejects the whole declaration instead of truncating: silently
/// dropping one would leave an app believing it published something the user
/// cannot see. Anything that must always be reachable belongs in the footer,
/// which is unbounded and scrolls.
pub const MAX_HEADER_SIDEBAR_ACTIONS: usize = 2;

/// A `value` names the context the person is working in (brand, store,
/// account). Its point is being read at a glance, so the footer carries only a
/// couple; more would turn shared chrome into a status panel.
pub const MAX_SIDEBAR_ACTION_VALUES: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SidebarActionPlacement {
    Header,
    Footer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellSidebarAction {
    pub id: String,
    pub placement: SidebarActionPlacement,
    pub label: String,
    pub icon: String,
    /// The changing fact the action shows (the working brand, store, account).
    /// Footer only; empty normalizes to `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default)]
    pub disabled: bool,
}

impl ShellSidebarAction {
    pub fn validate(mut self) -> ShellResult<Self> {
        self.id = required(self.id, ShellError::EmptySidebarActionId)?;
        self.label = required_field(self.label, "label")?;
        self.icon = required_field(self.icon, "icon")?;
        self.value = normalized_value(self.value);
        if self.value.is_some() && self.placement == SidebarActionPlacement::Header {
            return Err(ShellError::SidebarActionValueInHeader { id: self.id });
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShellSidebarActionUpdate {
    pub label: Option<String>,
    pub icon: Option<String>,
    /// `Some(None)` clears the value; `None` leaves it unchanged.
    pub value: Option<Option<String>>,
    pub disabled: Option<bool>,
}

impl ShellSidebarActionUpdate {
    fn validate(mut self, id: &str) -> ShellResult<Self> {
        if self.label.is_none()
            && self.icon.is_none()
            && self.value.is_none()
            && self.disabled.is_none()
        {
            return Err(ShellError::EmptySidebarActionUpdate { id: id.to_string() });
        }
        self.label = optional(self.label, "label")?;
        self.icon = optional(self.icon, "icon")?;
        self.value = self.value.map(normalized_value);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedShellSidebarAction {
    pub generation: u64,
    pub id: String,
    pub placement: SidebarActionPlacement,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<String>,
    pub disabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SidebarActionCollection {
    generation: u64,
    declared: bool,
    items: Vec<ShellSidebarAction>,
}

impl SidebarActionCollection {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn declared(&self) -> bool {
        self.declared
    }

    pub fn items(&self) -> &[ShellSidebarAction] {
        &self.items
    }

    pub fn replace(&mut self, items: Vec<ShellSidebarAction>) -> ShellResult<()> {
        let items = validate_generation(items)?;
        warn_on_indistinguishable_icons(&items);
        self.items = items;
        self.declared = true;
        self.generation = self.generation.wrapping_add(1);
        Ok(())
    }

    pub fn update(&mut self, id: &str, patch: ShellSidebarActionUpdate) -> ShellResult<()> {
        let id = id.trim();
        if id.is_empty() {
            return Err(ShellError::EmptySidebarActionId);
        }
        let patch = patch.validate(id)?;
        // Only an icon or value change can make two footer icons collide; a
        // label or disabled patch must not repeat the warning on every call.
        let changes_icons = patch.icon.is_some() || patch.value.is_some();
        let mut items = self.items.clone();
        let Some(item) = items.iter_mut().find(|item| item.id == id) else {
            return Err(ShellError::SidebarActionNotFound { id: id.to_string() });
        };
        if let Some(label) = patch.label {
            item.label = label;
        }
        if let Some(icon) = patch.icon {
            item.icon = icon;
        }
        if let Some(value) = patch.value {
            item.value = value;
        }
        if let Some(disabled) = patch.disabled {
            item.disabled = disabled;
        }
        // A patch can break a generation-wide rule (a header value, a third
        // value), so the patched generation is validated as a whole.
        self.items = validate_generation(items)?;
        if changes_icons {
            warn_on_indistinguishable_icons(&self.items);
        }
        self.declared = true;
        self.generation = self.generation.wrapping_add(1);
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> ShellResult<()> {
        let id = id.trim();
        if id.is_empty() {
            return Err(ShellError::EmptySidebarActionId);
        }
        let before = self.items.len();
        self.items.retain(|item| item.id != id);
        if self.items.len() == before {
            return Err(ShellError::SidebarActionNotFound { id: id.to_string() });
        }
        self.declared = true;
        self.generation = self.generation.wrapping_add(1);
        Ok(())
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.declared = true;
        self.generation = self.generation.wrapping_add(1);
    }
}

fn required(value: String, error: ShellError) -> ShellResult<String> {
    let value = value.trim();
    if value.is_empty() {
        Err(error)
    } else {
        Ok(value.to_string())
    }
}

fn required_field(value: String, field: &'static str) -> ShellResult<String> {
    let value = value.trim();
    if value.is_empty() {
        Err(ShellError::EmptySidebarActionField { field })
    } else {
        Ok(value.to_string())
    }
}

fn optional(value: Option<String>, field: &'static str) -> ShellResult<Option<String>> {
    value.map(|value| required_field(value, field)).transpose()
}

fn normalized_value(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn validate_generation(items: Vec<ShellSidebarAction>) -> ShellResult<Vec<ShellSidebarAction>> {
    let mut ids = HashSet::with_capacity(items.len());
    let items = items
        .into_iter()
        .map(ShellSidebarAction::validate)
        .map(|result| {
            let item = result?;
            if !ids.insert(item.id.clone()) {
                return Err(ShellError::DuplicateSidebarActionId { id: item.id });
            }
            Ok(item)
        })
        .collect::<ShellResult<Vec<_>>>()?;
    if items
        .iter()
        .filter(|item| item.placement == SidebarActionPlacement::Header)
        .count()
        > MAX_HEADER_SIDEBAR_ACTIONS
    {
        return Err(ShellError::SidebarActionHeaderLimit {
            max: MAX_HEADER_SIDEBAR_ACTIONS,
        });
    }
    let valued: Vec<String> = items
        .iter()
        .filter(|item| item.value.is_some())
        .map(|item| item.id.clone())
        .collect();
    if valued.len() > MAX_SIDEBAR_ACTION_VALUES {
        return Err(ShellError::SidebarActionValueLimit {
            max: MAX_SIDEBAR_ACTION_VALUES,
            ids: valued.join(", "),
            count: valued.len(),
        });
    }
    Ok(items)
}

/// Icon-only footer actions are told apart by their icon alone; two with the
/// same icon look identical. Not an error — the label still reaches the
/// tooltip — but almost always a mistake.
fn warn_on_indistinguishable_icons(items: &[ShellSidebarAction]) {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for item in items
        .iter()
        .filter(|item| item.placement == SidebarActionPlacement::Footer && item.value.is_none())
    {
        if let Some((other, _)) = seen.iter().find(|(_, icon)| *icon == item.icon) {
            log::warn!(
                "sidebar actions '{other}' and '{}' share icon '{}' and look identical in the footer; give each a distinct icon",
                item.id,
                item.icon
            );
        } else {
            seen.push((item.id.as_str(), item.icon.as_str()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(id: &str) -> ShellSidebarAction {
        ShellSidebarAction {
            id: id.to_string(),
            placement: SidebarActionPlacement::Footer,
            label: format!("Label {id}"),
            icon: "icons/action.svg".to_string(),
            value: None,
            disabled: false,
        }
    }

    fn valued(id: &str, value: &str) -> ShellSidebarAction {
        ShellSidebarAction {
            value: Some(value.to_string()),
            ..action(id)
        }
    }

    #[test]
    fn empty_value_normalizes_to_none() {
        let item = ShellSidebarAction {
            value: Some("  ".to_string()),
            ..action("brand")
        };
        assert_eq!(item.validate().unwrap().value, None);
    }

    #[test]
    fn header_value_rejects_the_generation() {
        let mut state = SidebarActionCollection::default();
        let mut header = valued("brand", "Acme");
        header.placement = SidebarActionPlacement::Header;

        assert_eq!(
            state.replace(vec![header]),
            Err(ShellError::SidebarActionValueInHeader {
                id: "brand".to_string()
            })
        );
        assert_eq!(state.generation(), 0);
    }

    #[test]
    fn a_third_value_rejects_the_generation() {
        let mut state = SidebarActionCollection::default();
        let result = state.replace(vec![
            valued("brand", "Acme"),
            valued("store", "Downtown"),
            valued("account", "Ada"),
        ]);

        assert_eq!(
            result,
            Err(ShellError::SidebarActionValueLimit {
                max: MAX_SIDEBAR_ACTION_VALUES,
                ids: "brand, store, account".to_string(),
                count: 3,
            })
        );
    }

    #[test]
    fn update_sets_and_clears_a_value() {
        let mut state = SidebarActionCollection::default();
        state.replace(vec![action("brand")]).unwrap();

        state
            .update(
                "brand",
                ShellSidebarActionUpdate {
                    value: Some(Some("Acme".to_string())),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(state.items()[0].value.as_deref(), Some("Acme"));

        state
            .update(
                "brand",
                ShellSidebarActionUpdate {
                    value: Some(None),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(state.items()[0].value, None);
    }

    #[test]
    fn update_cannot_break_the_value_rules() {
        let mut state = SidebarActionCollection::default();
        let mut header = action("downloads");
        header.placement = SidebarActionPlacement::Header;
        state
            .replace(vec![
                header,
                valued("brand", "Acme"),
                valued("store", "Downtown"),
                action("account"),
            ])
            .unwrap();
        let before = state.clone();

        let set = |value: &str| ShellSidebarActionUpdate {
            value: Some(Some(value.to_string())),
            ..Default::default()
        };
        assert_eq!(
            state.update("downloads", set("x")),
            Err(ShellError::SidebarActionValueInHeader {
                id: "downloads".to_string()
            })
        );
        assert!(matches!(
            state.update("account", set("Ada")),
            Err(ShellError::SidebarActionValueLimit { .. })
        ));
        assert_eq!(state, before);
    }

    #[test]
    fn replace_is_atomic_when_a_later_item_is_invalid() {
        let mut state = SidebarActionCollection::default();
        state.replace(vec![action("chat")]).unwrap();
        let before = state.clone();

        let result = state.replace(vec![action("ok"), action("")]);

        assert_eq!(result, Err(ShellError::EmptySidebarActionId));
        assert_eq!(state, before);
    }

    #[test]
    fn clear_is_an_explicit_empty_declaration() {
        let mut state = SidebarActionCollection::default();
        state.clear();

        assert!(state.declared());
        assert!(state.items().is_empty());
    }

    #[test]
    fn label_and_icon_are_required() {
        let mut missing_label = action("sync");
        missing_label.label.clear();
        assert_eq!(
            missing_label.validate(),
            Err(ShellError::EmptySidebarActionField { field: "label" })
        );

        let mut missing_icon = action("sync");
        missing_icon.icon.clear();
        assert_eq!(
            missing_icon.validate(),
            Err(ShellError::EmptySidebarActionField { field: "icon" })
        );
    }

    #[test]
    fn stable_ids_are_unique() {
        let mut state = SidebarActionCollection::default();
        let result = state.replace(vec![action("same"), action("same")]);

        assert_eq!(
            result,
            Err(ShellError::DuplicateSidebarActionId {
                id: "same".to_string()
            })
        );
    }

    #[test]
    fn the_header_limit_rejects_rather_than_truncates() {
        let mut state = SidebarActionCollection::default();
        let over_limit: Vec<_> = (0..=MAX_HEADER_SIDEBAR_ACTIONS)
            .map(|index| {
                let mut item = action(&format!("header-{index}"));
                item.placement = SidebarActionPlacement::Header;
                item
            })
            .collect();

        assert_eq!(
            state.replace(over_limit),
            Err(ShellError::SidebarActionHeaderLimit {
                max: MAX_HEADER_SIDEBAR_ACTIONS
            })
        );
        assert_eq!(state.generation(), 0);
    }

    /// The limit itself must be usable — a declaration exactly at it commits.
    #[test]
    fn the_header_limit_is_inclusive() {
        let mut state = SidebarActionCollection::default();
        let at_limit: Vec<_> = (0..MAX_HEADER_SIDEBAR_ACTIONS)
            .map(|index| {
                let mut item = action(&format!("header-{index}"));
                item.placement = SidebarActionPlacement::Header;
                item
            })
            .collect();

        assert!(state.replace(at_limit).is_ok());
        assert_eq!(state.generation(), 1);
    }
}
