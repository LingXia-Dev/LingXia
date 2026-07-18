use crate::{ShellError, ShellResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellActivator {
    pub id: String,
    pub label: String,
    pub icon: String,
    #[serde(default)]
    pub disabled: bool,
}

impl ShellActivator {
    pub fn validate(mut self) -> ShellResult<Self> {
        self.id = required(self.id, ShellError::EmptyActivatorId)?;
        self.label = required_field(self.label, "label")?;
        self.icon = required_field(self.icon, "icon")?;
        Ok(self)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShellActivatorUpdate {
    pub label: Option<String>,
    pub icon: Option<String>,
    pub disabled: Option<bool>,
}

impl ShellActivatorUpdate {
    fn validate(mut self, id: &str) -> ShellResult<Self> {
        if self.label.is_none() && self.icon.is_none() && self.disabled.is_none() {
            return Err(ShellError::EmptyActivatorUpdate { id: id.to_string() });
        }
        self.label = optional(self.label, "label")?;
        self.icon = optional(self.icon, "icon")?;
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedShellActivator {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<String>,
    pub disabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActivatorCollection {
    generation: u64,
    declared: bool,
    items: Vec<ShellActivator>,
}

impl ActivatorCollection {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn declared(&self) -> bool {
        self.declared
    }

    pub fn items(&self) -> &[ShellActivator] {
        &self.items
    }

    pub fn replace(&mut self, items: Vec<ShellActivator>) -> ShellResult<()> {
        let items = validate_generation(items)?;
        self.items = items;
        self.declared = true;
        self.generation = self.generation.wrapping_add(1);
        Ok(())
    }

    pub fn update(&mut self, id: &str, patch: ShellActivatorUpdate) -> ShellResult<()> {
        let id = id.trim();
        if id.is_empty() {
            return Err(ShellError::EmptyActivatorId);
        }
        let patch = patch.validate(id)?;
        let Some(item) = self.items.iter_mut().find(|item| item.id == id) else {
            return Err(ShellError::ActivatorNotFound { id: id.to_string() });
        };
        if let Some(label) = patch.label {
            item.label = label;
        }
        if let Some(icon) = patch.icon {
            item.icon = icon;
        }
        if let Some(disabled) = patch.disabled {
            item.disabled = disabled;
        }
        self.declared = true;
        self.generation = self.generation.wrapping_add(1);
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> ShellResult<()> {
        let id = id.trim();
        if id.is_empty() {
            return Err(ShellError::EmptyActivatorId);
        }
        let before = self.items.len();
        self.items.retain(|item| item.id != id);
        if self.items.len() == before {
            return Err(ShellError::ActivatorNotFound { id: id.to_string() });
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
        Err(ShellError::EmptyActivatorField { field })
    } else {
        Ok(value.to_string())
    }
}

fn optional(value: Option<String>, field: &'static str) -> ShellResult<Option<String>> {
    value.map(|value| required_field(value, field)).transpose()
}

fn validate_generation(items: Vec<ShellActivator>) -> ShellResult<Vec<ShellActivator>> {
    let mut ids = HashSet::with_capacity(items.len());
    items
        .into_iter()
        .map(ShellActivator::validate)
        .map(|result| {
            let item = result?;
            if !ids.insert(item.id.clone()) {
                return Err(ShellError::DuplicateActivatorId { id: item.id });
            }
            Ok(item)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activator(id: &str) -> ShellActivator {
        ShellActivator {
            id: id.to_string(),
            label: format!("Label {id}"),
            icon: "icons/activator.svg".to_string(),
            disabled: false,
        }
    }

    #[test]
    fn replace_is_atomic_when_a_later_item_is_invalid() {
        let mut state = ActivatorCollection::default();
        state.replace(vec![activator("chat")]).unwrap();
        let before = state.clone();

        let result = state.replace(vec![activator("ok"), activator("")]);

        assert_eq!(result, Err(ShellError::EmptyActivatorId));
        assert_eq!(state, before);
    }

    #[test]
    fn clear_is_an_explicit_empty_declaration() {
        let mut state = ActivatorCollection::default();
        state.clear();

        assert!(state.declared());
        assert!(state.items().is_empty());
    }

    #[test]
    fn label_and_icon_are_required() {
        let mut missing_label = activator("sync");
        missing_label.label.clear();
        assert_eq!(
            missing_label.validate(),
            Err(ShellError::EmptyActivatorField { field: "label" })
        );

        let mut missing_icon = activator("sync");
        missing_icon.icon.clear();
        assert_eq!(
            missing_icon.validate(),
            Err(ShellError::EmptyActivatorField { field: "icon" })
        );
    }

    #[test]
    fn stable_ids_are_unique() {
        let mut state = ActivatorCollection::default();
        let result = state.replace(vec![activator("same"), activator("same")]);

        assert_eq!(
            result,
            Err(ShellError::DuplicateActivatorId {
                id: "same".to_string()
            })
        );
    }
}
