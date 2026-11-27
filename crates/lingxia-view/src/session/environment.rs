use super::*;

impl ViewSession {
    /// Whether the View's last run read a host-context key; true until it has run.
    pub fn reads_host_context(&self) -> bool {
        self.reads_host_context
    }

    /// Whether the View's last run depends on the appearance; true until it has run.
    pub fn reads_appearance(&self) -> bool {
        self.reads_appearance
    }

    /// The finest `useNow` step the View's last run read, in ms; 0 when it read none.
    pub fn clock_step(&self) -> u64 {
        self.clock_step
    }

    /// The View's clock, epoch ms: the wall clock unless a test pinned it.
    pub(crate) fn clock_now(&self) -> u64 {
        self.clock_pin.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_millis() as u64)
        })
    }

    pub(crate) fn set_clock_pin(&mut self, pin: Option<u64>) {
        self.clock_pin = pin;
    }

    /// Moves `useNow` to the clock; true when a step the page shows changed, so it renders.
    pub fn clock_tick(&mut self) -> Result<bool, String> {
        let now = self.clock_now();
        self.execute(|| {
            self.context
                .eval::<bool>(Source::from_bytes(
                    format!("__lingxiaSetNow({now})").as_bytes(),
                ))
                .map_err(|e| crate::engine::js_error(&self.context, e))
        })
    }

    /// Native measurements are sampled with the frame; viewport stays renderer-owned.
    pub fn set_environment(&mut self, mut value: serde_json::Value) -> Result<(), String> {
        // Hover styles apply only to fine pointers; adapters that cannot tell report touch.
        if value.is_object() && value.get("pointer").is_none() {
            value["pointer"] = serde_json::json!("coarse");
        }
        let valid = crate::protocol::check_environment(&value);
        if !valid {
            return Err("UI_ENVIRONMENT: invalid native snapshot".into());
        }
        let typed: crate::protocol::env::Environment = serde_json::from_value(value.clone())
            .map_err(|error| format!("UI_ENVIRONMENT: {error}"))?;
        // Platforms that cannot report the setting leave motion on.
        if value.get("reducedMotion").is_none() {
            value["reducedMotion"] = serde_json::json!(false);
        }
        if value.get("highContrast").is_none() {
            value["highContrast"] = serde_json::json!(false);
        }
        if value.get("animatorDurationScale").is_none() {
            value["animatorDurationScale"] = serde_json::json!(1);
        }
        // A zero animator scale is the platform's "remove animations".
        if value["animatorDurationScale"] == 0.0 {
            value["reducedMotion"] = serde_json::json!(true);
        }
        // Direction follows the lxapp's text and is exposed as `env.direction`.
        let rtl = crate::text::fonts::right_to_left(value["language"].as_str().unwrap_or(""));
        value["direction"] = serde_json::json!(if rtl { "rtl" } else { "ltr" });
        if self.renderer.rtl != rtl {
            self.renderer.rtl = rtl;
            self.renderer.clear_text_cache();
            self.retained = None;
            // Committed records stay flow-relative, so a native frame can mirror them without the View.
            self.input_dirty = true;
            crate::layout::list_layout::invalidate_measurements(&mut self.lists);
        }
        let dark = typed.appearance == crate::protocol::env::Appearance::Dark;
        let high_contrast = typed.high_contrast == Some(true);
        // Pages never read colours: the palette is taken out before the View sees the snapshot.
        let palette = match value
            .as_object_mut()
            .and_then(|object| object.remove("palette"))
        {
            Some(pushed) => crate::theme::Palette::decode(&pushed)?.for_look(dark),
            None => crate::theme::Palette::builtin_for(dark, high_contrast),
        };
        if self.renderer.dark != dark || self.renderer.palette != palette {
            self.renderer.dark = dark;
            self.renderer.palette = palette;
            self.retained = None;
            // Colours are roles the renderer resolves: the committed records repaint in the new
            // palette without the View.
            self.input_dirty = true;
        }
        // Theme scales, like the palette, reach the renderer and never the View.
        let scales = match value
            .as_object_mut()
            .and_then(|object| object.remove("scales"))
        {
            Some(pushed) => std::sync::Arc::new(crate::protocol::scales::Scales::decode(&pushed)?),
            None => crate::protocol::scales::Scales::builtin(),
        };
        if *self.scales != *scales {
            self.rescale(scales)?;
        }
        let text_scale = crate::text::scale::TextScale::decode(value.get("textScale"))?;
        if self.renderer.text_scale != text_scale {
            self.renderer.text_scale = text_scale;
            self.retained = None;
            crate::layout::list_layout::invalidate_measurements(&mut self.lists);
            self.reveal_input = true;
        }
        let locale = value["language"]
            .as_str()
            .and_then(crate::text::fonts::locale);
        if self.renderer.locale != locale {
            self.renderer.locale = locale;
            self.renderer.clear_text_cache();
            self.retained = None;
            crate::layout::list_layout::invalidate_measurements(&mut self.lists);
        }
        let insets = typed.insets;
        let safe_area = crate::layout::style::SafeArea {
            insets: [insets.top, insets.right, insets.bottom, insets.left].map(|n| n as f32),
            keyboard: typed.keyboard.height as f32,
        };
        if self.renderer.safe_area != safe_area {
            if self.renderer.safe_area.keyboard != safe_area.keyboard {
                log::info!(
                    target: "lingxia_view",
                    "keyboard inset: {} -> {} (bottom inset {})",
                    self.renderer.safe_area.keyboard, safe_area.keyboard, safe_area.insets[2]
                );
            }
            self.renderer.safe_area = safe_area;
            self.retained = None;
        }
        self.reveal_input |= self.environment["insets"] != value["insets"]
            || self.environment["keyboard"] != value["keyboard"];
        // A change that re-renders the whole page by design (appearance, language, text size)
        // gets a first render's allowance. Insets, the keyboard and the viewport move with every
        // frame of an animation and get none.
        let steady = |key: &str| matches!(key, "insets" | "keyboard" | "viewport" | "safeArea");
        if let (Some(old), Some(new)) = (self.environment.as_object(), value.as_object())
            && (old.len() != new.len()
                || new
                    .iter()
                    .any(|(key, value)| !steady(key) && old.get(key) != Some(value)))
        {
            self.warm.set(true);
        }
        let intl = crate::engine::intl::Settings {
            language: value["language"].as_str().map(str::to_owned),
            hour_cycle: value["hourCycle"].as_str().map(str::to_owned),
        };
        if self.intl != intl {
            crate::engine::intl::update(&self.context, &intl);
            self.intl = intl;
        }
        // Without a language the source catalogue still loads: `t` needs one in the first frame.
        self.ensure_language_catalogue(value["language"].as_str().unwrap_or(""))?;
        self.environment = value;
        Ok(())
    }

    /// Synchronously evaluates the active and source-fallback per-language catalogue modules
    /// (`lx:view/<id>/<tag>`) into this isolate between frames when the page uses split catalogues.
    pub(super) fn ensure_language_catalogue(&mut self, language: &str) -> Result<(), String> {
        let (Some(prefix), Some(loader)) = (self.catalogue_chunk.clone(), self.chunks.clone())
        else {
            return Ok(());
        };
        let mut tags_to_load: Vec<String> = Vec::with_capacity(2);
        if let Some(source) = &self.catalogue_source {
            tags_to_load.push(source.clone());
        }
        if let Some(resolved) = crate::engine::chunks::resolve_catalogue_tag(
            language,
            &self.catalogue_languages,
            self.catalogue_source.as_deref(),
        ) && !tags_to_load.contains(&resolved)
        {
            tags_to_load.push(resolved);
        }
        for tag in tags_to_load {
            if self.loaded_languages.contains(&tag) {
                continue;
            }
            let (specifier, source) = crate::engine::chunks::load_language_module(
                &prefix,
                &tag,
                &loader,
                self.chunk_build_id.as_deref(),
            )?;
            let (outcome, _) =
                self.budget
                    .run(Duration::from_secs(1), Duration::from_secs(1), || {
                        <crate::engine::Active as crate::engine::Engine>::evaluate_auxiliary_module(
                        &self.context,
                        &specifier,
                        &source,
                    )?;
                        <crate::engine::Active as crate::engine::Engine>::drain_jobs(&self.context)
                    });
            outcome?;
            self.loaded_languages.insert(tag);
        }
        Ok(())
    }

    /// Committed records resolved again under `scales`: the same work as a text-scale change,
    /// without running the View.
    fn rescale(
        &mut self,
        scales: std::sync::Arc<crate::protocol::scales::Scales>,
    ) -> Result<(), String> {
        if let Some(records) = &self.input_records
            && let Some(rescaled) = crate::protocol::scales::rescale(records, &scales)?
        {
            self.input_records = Some(rescaled);
        }
        self.wire.borrow_mut().set_scales(scales.clone())?;
        self.renderer.control_radius = scales.control_radius();
        self.scales = scales;
        self.paint_records = None;
        self.renderer.clear_text_cache();
        self.retained = None;
        crate::layout::list_layout::invalidate_measurements(&mut self.lists);
        self.input_dirty = true;
        Ok(())
    }

    /// The pressed and hovered nodes the adapter reports; paint deltas follow without running the View.
    pub fn set_pointer_state(&mut self, pressed: Option<&str>, hovered: Option<&str>) -> bool {
        let changed = self.pressed.as_deref() != pressed || self.hovered.as_deref() != hovered;
        if changed {
            self.pressed = pressed.map(str::to_owned);
            self.hovered = hovered.map(str::to_owned);
            self.pointer_dirty = true;
        }
        changed
    }

    /// The styled control holding keyboard focus; its `focus:` delta repaints without the View.
    pub fn set_control_focus(&mut self, identity: Option<&str>) -> bool {
        self.set_control_focus_visible(identity, true)
    }

    /// As [`Self::set_control_focus`]; focus a pointer moved applies `focus:` but neither
    /// `focus-visible:` nor an indicator's ring.
    pub fn set_control_focus_visible(&mut self, identity: Option<&str>, visible: bool) -> bool {
        let changed =
            self.focused_control.as_deref() != identity || self.renderer.focus_visible != visible;
        if changed {
            self.focused_control = identity.map(str::to_owned);
            self.renderer.focus_visible = visible;
            self.input_dirty = true;
        }
        changed
    }

    pub(super) fn apply_pointer_styles(
        &self,
        records: &mut [crate::protocol::Record],
    ) -> Result<(), String> {
        if let Some(identity) = &self.focused_control {
            crate::interaction::input_state::apply_state_style(
                records,
                identity,
                crate::protocol::field::FOCUS_STYLE,
            )?;
            if self.renderer.focus_visible {
                crate::interaction::input_state::apply_state_style(
                    records,
                    identity,
                    crate::protocol::field::FOCUS_VISIBLE_STYLE,
                )?;
            }
            crate::interaction::input_state::apply_focus_within(records, identity)?;
        }
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
            crate::interaction::input_state::apply_group_pressed(records, identity)?;
        }
        Ok(())
    }
}
