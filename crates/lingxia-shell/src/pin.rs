use crate::{ShellError, ShellResult};
use serde::{Deserialize, Serialize};

pub const MAX_SHELL_PINS: usize = 8;
pub(crate) const PIN_STATE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ShellPinTarget {
    Lxapp { key: String },
    Bookmark { key: String },
}

impl ShellPinTarget {
    fn validate(mut self) -> ShellResult<Self> {
        let key = match &mut self {
            Self::Lxapp { key } | Self::Bookmark { key } => key,
        };
        *key = key.trim().to_string();
        if key.is_empty() {
            return Err(ShellError::InvalidState(
                "shell Pin target must not be empty".to_string(),
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ShellPin(pub ShellPinTarget);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinMutation {
    Changed,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinCollection {
    pub version: u32,
    pub items: Vec<ShellPin>,
}

impl Default for PinCollection {
    fn default() -> Self {
        Self {
            version: PIN_STATE_VERSION,
            items: Vec::new(),
        }
    }
}

impl PinCollection {
    pub fn restore(mut self) -> ShellResult<Self> {
        if self.version != PIN_STATE_VERSION {
            return Err(ShellError::UnsupportedVersion {
                version: self.version,
            });
        }
        let mut normalized = Vec::with_capacity(self.items.len());
        for pin in self.items {
            let pin = ShellPin(pin.0.validate()?);
            if normalized.contains(&pin) {
                return Err(ShellError::InvalidState(
                    "shell Pin store contains duplicate targets".to_string(),
                ));
            }
            normalized.push(pin);
            if normalized.len() > MAX_SHELL_PINS {
                return Err(ShellError::LimitReached {
                    max: MAX_SHELL_PINS,
                });
            }
        }
        self.items = normalized;
        Ok(self)
    }

    pub fn pin(&mut self, target: ShellPinTarget) -> ShellResult<PinMutation> {
        let pin = ShellPin(target.validate()?);
        if self.items.contains(&pin) {
            return Ok(PinMutation::Unchanged);
        }
        if self.items.len() >= MAX_SHELL_PINS {
            return Err(ShellError::LimitReached {
                max: MAX_SHELL_PINS,
            });
        }
        self.items.push(pin);
        Ok(PinMutation::Changed)
    }

    pub fn unpin(&mut self, target: &ShellPinTarget) -> PinMutation {
        let before = self.items.len();
        self.items.retain(|pin| &pin.0 != target);
        if self.items.len() == before {
            PinMutation::Unchanged
        } else {
            PinMutation::Changed
        }
    }

    pub fn is_pinned(&self, target: &ShellPinTarget) -> bool {
        self.items.iter().any(|pin| &pin.0 == target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lxapp(index: usize) -> ShellPinTarget {
        ShellPinTarget::Lxapp {
            key: format!("app.{index}"),
        }
    }

    #[test]
    fn mixed_pin_order_is_preserved() {
        let mut pins = PinCollection::default();
        pins.pin(lxapp(1)).unwrap();
        pins.pin(ShellPinTarget::Bookmark {
            key: "bookmark-a".to_string(),
        })
        .unwrap();
        pins.pin(lxapp(2)).unwrap();

        assert!(matches!(pins.items[0].0, ShellPinTarget::Lxapp { .. }));
        assert!(matches!(pins.items[1].0, ShellPinTarget::Bookmark { .. }));
        assert!(matches!(pins.items[2].0, ShellPinTarget::Lxapp { .. }));
    }

    #[test]
    fn ninth_pin_returns_typed_limit_error() {
        let mut pins = PinCollection::default();
        for index in 0..MAX_SHELL_PINS {
            pins.pin(lxapp(index)).unwrap();
        }

        assert_eq!(
            pins.pin(lxapp(MAX_SHELL_PINS)),
            Err(ShellError::LimitReached {
                max: MAX_SHELL_PINS
            })
        );
        assert_eq!(pins.items.len(), MAX_SHELL_PINS);
    }

    #[test]
    fn restore_rejects_overflow_instead_of_migrating_it() {
        let stored = PinCollection {
            version: PIN_STATE_VERSION,
            items: (0..=MAX_SHELL_PINS)
                .map(|index| ShellPin(lxapp(index)))
                .collect(),
        };

        assert_eq!(
            stored.restore(),
            Err(ShellError::LimitReached {
                max: MAX_SHELL_PINS,
            })
        );
    }
}
