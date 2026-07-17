use crate::{ShellError, ShellResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub(crate) const ACTIVATOR_STATE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivatorKind {
    Lxapp,
    Native,
    Action,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NativeShellCapability {
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ShellActivatorTarget {
    Lxapp { key: String },
    Native { key: NativeShellCapability },
    Action,
}

impl ShellActivatorTarget {
    pub fn kind(&self) -> ActivatorKind {
        match self {
            Self::Lxapp { .. } => ActivatorKind::Lxapp,
            Self::Native { .. } => ActivatorKind::Native,
            Self::Action => ActivatorKind::Action,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellActivator {
    pub id: String,
    pub target: ShellActivatorTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default)]
    pub disabled: bool,
}

impl ShellActivator {
    pub fn validate(mut self) -> ShellResult<Self> {
        self.id = required(self.id, ShellError::EmptyActivatorId)?;
        match &mut self.target {
            ShellActivatorTarget::Lxapp { key } => {
                *key = required(key.clone(), ShellError::EmptyActivatorTarget)?;
            }
            ShellActivatorTarget::Native { .. } => {}
            ShellActivatorTarget::Action => {}
        }
        self.label = optional(self.label, "label")?;
        self.icon = optional(self.icon, "icon")?;
        if matches!(self.target, ShellActivatorTarget::Action)
            && (self.label.is_none() || self.icon.is_none())
        {
            return Err(ShellError::IncompleteAction {
                id: self.id.clone(),
            });
        }
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
    pub kind: ActivatorKind,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<String>,
    pub active: bool,
    pub disabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedActivatorSnapshot {
    pub declared: bool,
    pub items: Vec<ResolvedShellActivator>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivatorDeclaration {
    pub version: u32,
    pub declared: bool,
    pub items: Vec<ShellActivator>,
}

impl Default for ActivatorDeclaration {
    fn default() -> Self {
        Self {
            version: ACTIVATOR_STATE_VERSION,
            declared: false,
            items: Vec::new(),
        }
    }
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
            item.label = Some(label);
        }
        if let Some(icon) = patch.icon {
            item.icon = Some(icon);
        }
        if let Some(disabled) = patch.disabled {
            item.disabled = disabled;
        }
        if matches!(item.target, ShellActivatorTarget::Action)
            && (item.label.is_none() || item.icon.is_none())
        {
            return Err(ShellError::IncompleteAction {
                id: item.id.clone(),
            });
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

    pub fn declaration(&self) -> ActivatorDeclaration {
        ActivatorDeclaration {
            version: ACTIVATOR_STATE_VERSION,
            declared: self.declared,
            items: self
                .items
                .iter()
                .filter(|item| !matches!(item.target, ShellActivatorTarget::Action))
                .cloned()
                .collect(),
        }
    }

    pub fn restore(declaration: ActivatorDeclaration) -> ShellResult<Self> {
        if declaration.version != ACTIVATOR_STATE_VERSION {
            return Err(ShellError::UnsupportedVersion {
                version: declaration.version,
            });
        }
        let items = validate_generation(
            declaration
                .items
                .into_iter()
                .filter(|item| !matches!(item.target, ShellActivatorTarget::Action))
                .collect(),
        )?;
        Ok(Self {
            generation: u64::from(declaration.declared),
            declared: declaration.declared,
            items,
        })
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

fn optional(value: Option<String>, field: &'static str) -> ShellResult<Option<String>> {
    value
        .map(|value| {
            let value = value.trim();
            if value.is_empty() {
                Err(ShellError::EmptyActivatorField { field })
            } else {
                Ok(value.to_string())
            }
        })
        .transpose()
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

    fn lxapp(id: &str, key: &str) -> ShellActivator {
        ShellActivator {
            id: id.to_string(),
            target: ShellActivatorTarget::Lxapp {
                key: key.to_string(),
            },
            label: None,
            icon: None,
            disabled: false,
        }
    }

    #[test]
    fn replace_is_atomic_when_a_later_item_is_invalid() {
        let mut state = ActivatorCollection::default();
        state.replace(vec![lxapp("chat", "app.chat")]).unwrap();
        let before = state.clone();

        let result = state.replace(vec![lxapp("ok", "app.ok"), lxapp("", "app.bad")]);

        assert_eq!(result, Err(ShellError::EmptyActivatorId));
        assert_eq!(state, before);
    }

    #[test]
    fn explicit_empty_declaration_survives_restore() {
        let mut state = ActivatorCollection::default();
        state.clear();

        let restored = ActivatorCollection::restore(state.declaration()).unwrap();

        assert!(restored.declared());
        assert!(restored.items().is_empty());
    }

    #[test]
    fn action_items_are_process_local() {
        let mut state = ActivatorCollection::default();
        state
            .replace(vec![ShellActivator {
                id: "sync".to_string(),
                target: ShellActivatorTarget::Action,
                label: Some("Sync".to_string()),
                icon: Some("icons/sync.svg".to_string()),
                disabled: false,
            }])
            .unwrap();

        let persisted = state.declaration();
        assert!(persisted.declared);
        assert!(persisted.items.is_empty());
    }

    #[test]
    fn stable_ids_are_unique_across_target_kinds() {
        let mut state = ActivatorCollection::default();
        let result = state.replace(vec![
            lxapp("same", "app.chat"),
            ShellActivator {
                id: "same".to_string(),
                target: ShellActivatorTarget::Native {
                    key: NativeShellCapability::Terminal,
                },
                label: None,
                icon: None,
                disabled: false,
            },
        ]);

        assert_eq!(
            result,
            Err(ShellError::DuplicateActivatorId {
                id: "same".to_string()
            })
        );
    }
}
