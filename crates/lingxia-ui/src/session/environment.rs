use super::*;

impl ViewSession {
    /// Native measurements are sampled with the frame; viewport stays renderer-owned.
    pub fn set_environment(&mut self, mut value: serde_json::Value) -> Result<(), String> {
        // Hover styles apply only to fine pointers; adapters that cannot tell report touch.
        if value.is_object() && value.get("pointer").is_none() {
            value["pointer"] = serde_json::json!("coarse");
        }
        let valid = value.is_object()
            && matches!(value["pointer"].as_str(), Some("fine" | "coarse"))
            && matches!(value["theme"].as_str(), Some("light" | "dark"))
            && ["top", "right", "bottom", "left"].iter().all(|edge| {
                value["insets"][edge]
                    .as_f64()
                    .is_some_and(|n| n.is_finite() && (0.0..=16384.0).contains(&n))
            })
            && value["keyboard"]["height"]
                .as_f64()
                .is_some_and(|n| n.is_finite() && (0.0..=16384.0).contains(&n))
            && ["os", "formFactor", "displayLanguage"].iter().all(|key| {
                value[key]
                    .as_str()
                    .is_some_and(|s| !s.is_empty() && s.len() <= 128)
            })
            && (value.get("sizeClass").is_none()
                || matches!(value["sizeClass"].as_str(), Some("compact" | "regular")))
            && value["canDock"].is_boolean()
            && value["runner"].is_boolean()
            && (value.get("renderer").is_none()
                || (matches!(
                    value["renderer"]["fallback"].as_str(),
                    None | Some("driver")
                ) && value["renderer"].as_object().is_some_and(|r| r.len() == 1)));
        if !valid {
            return Err("UI_ENVIRONMENT: invalid native snapshot".into());
        }
        let text_scale = crate::text::scale::TextScale::decode(value.get("textScale"))?;
        if self.renderer.text_scale != text_scale {
            self.renderer.text_scale = text_scale;
            self.retained = None;
            crate::layout::list_layout::invalidate_measurements(&mut self.lists);
            self.reveal_input = true;
        }
        let locale = value["displayLanguage"]
            .as_str()
            .and_then(crate::text::fonts::locale);
        if self.renderer.locale != locale {
            self.renderer.locale = locale;
            self.renderer.clear_text_cache();
            self.retained = None;
            crate::layout::list_layout::invalidate_measurements(&mut self.lists);
        }
        let edge = |name: &str| value["insets"][name].as_f64().unwrap_or(0.0) as f32;
        let safe_area = crate::layout::style::SafeArea {
            insets: [edge("top"), edge("right"), edge("bottom"), edge("left")],
            keyboard: value["keyboard"]["height"].as_f64().unwrap_or(0.0) as f32,
        };
        if self.renderer.safe_area != safe_area {
            self.renderer.safe_area = safe_area;
            self.retained = None;
        }
        self.reveal_input |= self.environment["insets"] != value["insets"]
            || self.environment["keyboard"] != value["keyboard"];
        self.environment = value;
        Ok(())
    }

    /// The pressed and hovered nodes the adapter reports; paint deltas follow without running the View.
    pub fn set_pointer_state(&mut self, pressed: Option<&str>, hovered: Option<&str>) -> bool {
        let changed = self.pressed.as_deref() != pressed || self.hovered.as_deref() != hovered;
        if changed {
            self.pressed = pressed.map(str::to_owned);
            self.hovered = hovered.map(str::to_owned);
            self.input_dirty = true;
        }
        changed
    }

    pub(super) fn apply_pointer_styles(
        &self,
        records: &mut [crate::protocol::Record],
    ) -> Result<(), String> {
        if let Some(identity) = &self.hovered {
            crate::interaction::input_state::apply_state_style(
                records,
                identity,
                crate::protocol::field::HOVER_STYLE,
            )?;
        }
        if let Some(identity) = &self.pressed {
            crate::interaction::input_state::apply_state_style(
                records,
                identity,
                crate::protocol::field::PRESSED_STYLE,
            )?;
        }
        Ok(())
    }
}
