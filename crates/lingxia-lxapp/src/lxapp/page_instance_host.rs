//! Host-side page instance orchestration for [`LxApp`].
//!
//! This module owns route resolution, page-instance lifecycle integration,
//! page stack operations, and view-call convenience APIs.

use super::*;

fn navigation_operation(nav_type: crate::page::NavigationType) -> &'static str {
    match nav_type {
        crate::page::NavigationType::Launch => "reLaunch",
        crate::page::NavigationType::Forward => "navigateTo",
        crate::page::NavigationType::Backward => "navigateBack",
        crate::page::NavigationType::Replace => "redirectTo",
        crate::page::NavigationType::SwitchTab => "switchTab",
    }
}

fn navigation_entry_error(
    reason: &'static str,
    nav_type: crate::page::NavigationType,
    target: &str,
    detail: String,
) -> LxAppError {
    LxAppError::RongJSHost {
        code: "E_INVALID_ARG".to_string(),
        message: detail.clone(),
        data: Some(serde_json::json!({
            "bizCode": 1002,
            "detail": detail,
            "reason": reason,
            "operation": navigation_operation(nav_type),
            "target": target,
        })),
    }
}

fn validate_navigation_stack(
    stack: &[String],
    pinned_instance: Option<&str>,
    target_path: &str,
    nav_type: crate::page::NavigationType,
) -> Result<(), LxAppError> {
    match nav_type {
        // Every forward entry mints its own instance, so the same route may
        // stack repeatedly. Only a path-pinned singleton (tab page) can
        // resolve to an instance that is already on the stack.
        crate::page::NavigationType::Forward
            if pinned_instance.is_some_and(|id| stack.iter().any(|entry| entry == id)) =>
        {
            Err(navigation_entry_error(
                "duplicate_route",
                nav_type,
                target_path,
                format!(
                    "navigateTo target '{target_path}' is already on the page stack; \
                     use lx.switchTab or lx.navigateBack to return to it."
                ),
            ))
        }
        crate::page::NavigationType::Forward if stack.len() >= PAGE_STACK_MAX => {
            Err(navigation_entry_error(
                "stack_full",
                nav_type,
                target_path,
                format!(
                    "navigateTo cannot open '{target_path}': the page stack is full \
                     (capacity: {PAGE_STACK_MAX})."
                ),
            ))
        }
        _ => Ok(()),
    }
}

impl LxApp {
    /// Find the actual configured page path that matches the given path.
    /// Returns the path with proper extension if found.
    pub fn find_page_path(&self, path: &str) -> Option<String> {
        let pages = self.config.page_paths();
        find_matching_page_path(&pages, path).map(|s| s.to_string())
    }

    pub fn find_page_path_by_name(&self, name: &str) -> Option<String> {
        self.config.page_path_by_name(name)
    }

    /// Validate that a page URL resolves to a configured page before navigation.
    pub fn ensure_page_exists(&self, url: &str) -> Result<(), LxAppError> {
        let resolved = crate::route::resolve_route(self, url)?;
        self.ensure_resolved_route_exists(&resolved)
    }

    fn ensure_resolved_route_exists(
        &self,
        resolved: &crate::route::ResolvedRoute,
    ) -> Result<(), LxAppError> {
        match &resolved.target {
            crate::route::RouteTarget::Normal { path } => {
                if self.is_configured_page(path) {
                    Ok(())
                } else {
                    Err(LxAppError::ResourceNotFound(path.clone()))
                }
            }
            crate::route::RouteTarget::Plugin { name, path } => {
                if self.is_plugin_page_configured(name, path, &resolved.original) {
                    Ok(())
                } else {
                    Err(LxAppError::ResourceNotFound(format!(
                        "plugin/{}/{}",
                        name, path
                    )))
                }
            }
        }
    }

    fn is_configured_page(&self, path: &str) -> bool {
        let pages = self.config.page_paths();
        !path.trim_start_matches('/').is_empty() && find_matching_page_path(&pages, path).is_some()
    }

    fn is_plugin_page_configured(
        &self,
        plugin_name: &str,
        resolved_page_path: &str,
        original_url: &str,
    ) -> bool {
        let plugin_cfg = match self.config.plugins.get(plugin_name) {
            Some(cfg) => cfg,
            None => return false,
        };

        let requested_path = extract_plugin_page_path(original_url)
            .unwrap_or_else(|| resolved_page_path.to_string());

        if !plugin_cfg.pages.is_empty() {
            return plugin_page_map_contains(
                &plugin_cfg.pages,
                &requested_path,
                resolved_page_path,
            );
        }

        if let Some(pages) =
            crate::plugin::load_plugin_manifest_pages(&self.runtime, plugin_name, plugin_cfg)
        {
            return plugin_page_map_contains(&pages, &requested_path, resolved_page_path);
        }

        true
    }

    fn build_page_target_url(
        &self,
        target: &PageTarget,
        query: Option<&PageQueryInput>,
    ) -> Result<String, LxAppError> {
        let base = match target {
            PageTarget::Name(name) => self
                .find_page_path_by_name(name.trim())
                .ok_or_else(|| LxAppError::ResourceNotFound(format!("page name: {}", name)))?,
            PageTarget::Path(path) => {
                let trimmed = path.trim();
                if trimmed.is_empty() {
                    self.config.get_initial_route()
                } else {
                    trimmed.to_string()
                }
            }
        };

        if base.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "page target path must not be empty".to_string(),
            ));
        }

        let Some(query) = query else {
            return Ok(base);
        };
        let query = query.to_query_string();
        if query.is_empty() {
            return Ok(base);
        }
        let separator = if base.contains('?') { '&' } else { '?' };
        Ok(format!("{base}{separator}{query}"))
    }

    fn page_definition_for_resolved_path(&self, resolved_path: &str) -> PageDefinition {
        let page_entries = self.config.page_entries();
        let matched_entry = page_entries
            .into_iter()
            .find(|entry| normalize_page_path(&entry.path) == normalize_page_path(resolved_path));

        let (name, config_path) = if let Some(entry) = matched_entry {
            (Some(entry.name), entry.path)
        } else {
            (None, resolved_path.to_string())
        };

        // Page configuration belongs to the native presenter as well as to a
        // PageSvc. A logic-disabled host still needs its declared chrome and
        // orientation represented in automation/runtime records.
        let config = PageConfig::from_json(self, &config_path).unwrap_or_default();

        PageDefinition {
            name,
            path: resolved_path.to_string(),
            config,
        }
    }

    fn fallback_runtime_record_for_page(&self, page: &PageInstance) -> PageInstanceRuntimeRecord {
        let path = page.path();
        PageInstanceRuntimeRecord {
            owner: PageOwner::Host,
            surface: PresentationKind::Window,
            dispose_ttl: None,
            page: ResolvedPage {
                appid: self.appid.clone(),
                path: path.clone(),
                query: String::new(),
                definition: self.page_definition_for_resolved_path(&path),
            },
            lifecycle: PageInstanceLifecycleState::Created,
        }
    }

    fn upsert_page_instance_runtime_record(
        &self,
        page: &PageInstance,
        owner: PageOwner,
        surface: PresentationKind,
        dispose_ttl: Option<std::time::Duration>,
        resolved: ResolvedPage,
    ) {
        if let Ok(state) = self.state.lock() {
            state.page_instance_runtime.lock().unwrap().insert(
                page.instance_id_string(),
                PageInstanceRuntimeRecord {
                    owner,
                    surface,
                    dispose_ttl,
                    page: resolved,
                    lifecycle: PageInstanceLifecycleState::Created,
                },
            );
        }
    }

    pub fn resolve_page_target(
        &self,
        target: &PageTarget,
        query: Option<&PageQueryInput>,
    ) -> Result<ResolvedPage, LxAppError> {
        let target_url = self.build_page_target_url(target, query)?;
        let resolved = crate::route::resolve_route(self, &target_url)?;
        self.ensure_resolved_route_exists(&resolved)?;
        let resolved_path = resolved.internal_path();
        let query = resolved.query.unwrap_or_default();

        Ok(ResolvedPage {
            appid: self.appid.clone(),
            path: resolved_path.clone(),
            query,
            definition: self.page_definition_for_resolved_path(&resolved_path),
        })
    }

    pub fn create_page_instance(
        &self,
        owner: PageOwner,
        target: PageTarget,
        query: Option<PageQueryInput>,
        surface: PresentationKind,
        dispose_ttl: Option<std::time::Duration>,
    ) -> Result<CreatedPageInstance, LxAppError> {
        let target_url = self.build_page_target_url(&target, query.as_ref())?;
        let resolved = self.resolve_page_target(&target, query.as_ref())?;

        // Keep AppService alive only for logic-enabled apps.
        if self.logic_enabled()
            && let Err(e) = self.executor.create_app_svc(self.clone_arc())
        {
            warn!(
                "Failed to ensure app service while creating page instance: {}",
                e
            )
            .with_appid(self.appid.clone());
        }

        let page = match &owner {
            PageOwner::Page(_) => {
                let page = self.create_isolated_page_instance(&resolved.path);
                if !resolved.query.is_empty() {
                    page.set_query(resolved.query.clone());
                }
                page
            }
            _ => {
                let resolved_path = crate::delegate::LxAppDelegate::on_lxapp_opened(
                    self.clone_arc(),
                    target_url,
                    self.session.id,
                );
                if resolved_path.is_empty() {
                    return Err(LxAppError::UnsupportedOperation(
                        "failed to open page instance for current session".to_string(),
                    ));
                }

                if let Some(page) = self.get_page(&resolved.path) {
                    page
                } else {
                    let page = self.get_or_create_page(&resolved.path);
                    if !resolved.query.is_empty() {
                        page.set_query(resolved.query.clone());
                    }
                    page
                }
            }
        };
        self.cancel_page_instance_dispose_timer(&page.instance_id());

        self.upsert_page_instance_runtime_record(
            &page,
            owner,
            surface,
            dispose_ttl,
            resolved.clone(),
        );

        Ok(CreatedPageInstance {
            page_instance_id: page.instance_id(),
            appid: self.appid.clone(),
            resolved_path: resolved.path,
            query: resolved.query,
        })
    }

    fn create_isolated_page_instance(&self, path: &str) -> PageInstance {
        let appid = self.appid.clone();
        let lxapp_arc = self.clone_arc();
        let waits_for_page_service = isolated_page_waits_for_page_service(self.logic_enabled());
        let page = PageInstance::new_with_isolation(
            appid.clone(),
            path.to_string(),
            self,
            true,
            move |page| {
                let lxapp_arc = lxapp_arc.clone();
                let page_clone = page.clone();
                async move {
                    if waits_for_page_service {
                        // The opener creates PageSvc on the JS worker (the same
                        // worker that is awaiting this page). Posting CreatePage
                        // from here would sit behind that wait forever.
                        page_clone.wait_page_svc_ready().await?;
                    }

                    page_clone
                        .load_html()
                        .map_err(|e| format!("Failed to load HTML for page: {}", e))?;
                    lxapp_arc
                        .notify_page_instance(&page_clone.instance_id(), PageInstanceEvent::Mounted)
                        .map_err(|e| format!("Failed to mount page instance: {}", e))?;
                    // Isolated pages are never `current_page()`, so sync_host_ui
                    // would leave chrome:full's drag-strip inset at 0.
                    let revision = lxapp_arc.next_page_chrome_revision();
                    let appearance = lxapp_arc.appearance_state().resolved;
                    if let Err(err) = lxapp_arc
                        .publish_realized_page_chrome(&page_clone, revision, appearance)
                        .await
                    {
                        warn!("Failed to publish isolated page chrome: {}", err)
                            .with_appid(lxapp_arc.appid.clone())
                            .with_path(page_clone.path());
                    }
                    Ok(())
                }
            },
        );

        let state = self.state.lock().unwrap();
        state
            .pages_by_id
            .lock()
            .unwrap()
            .insert(page.instance_id_string(), page.clone());
        page
    }

    pub fn notify_page_instance(
        &self,
        id: &PageInstanceId,
        event: PageInstanceEvent,
    ) -> Result<(), LxAppError> {
        let page = self.get_page_by_instance_id(id).ok_or_else(|| {
            LxAppError::ResourceNotFound(format!("page instance id: {}", id.as_str()))
        })?;
        let (
            owner_for_log,
            presentation_for_log,
            dispose_ttl,
            resolved_path_for_log,
            query_for_log,
            definition_path_for_log,
        ) = {
            let state = self.state.lock().unwrap();
            let mut records = state.page_instance_runtime.lock().unwrap();
            let record = records
                .entry(id.as_str().to_string())
                .or_insert_with(|| self.fallback_runtime_record_for_page(&page));
            record.lifecycle = transition_page_instance_lifecycle(record.lifecycle, &event)?;
            (
                record.owner.clone(),
                record.surface,
                record.dispose_ttl,
                record.page.path.clone(),
                record.page.query.clone(),
                record.page.definition.path.clone(),
            )
        };

        info!(
            "notify_page_instance id={} owner={:?} surface={:?} path={} query={} definition={} event={:?}",
            id,
            owner_for_log,
            presentation_for_log,
            resolved_path_for_log,
            query_for_log,
            definition_path_for_log,
            event
        )
        .with_appid(self.appid.clone())
        .with_path(page.path());

        match event {
            PageInstanceEvent::Mounted => {
                self.cancel_page_instance_dispose_timer(id);
            }
            PageInstanceEvent::Visible => {
                self.cancel_page_instance_dispose_timer(id);
                page.dispatch_lifecycle_event(crate::lifecycle::PageLifecycleEvent::OnShow);
                page.mark_active();
            }
            PageInstanceEvent::Hidden { reason } => {
                page.dispatch_lifecycle_event(crate::lifecycle::PageLifecycleEvent::OnHide);
                if matches!(reason, CloseReason::AppClosed) {
                    self.dispose_page_instance_internal(id, reason, false)?;
                } else if let Some(dispose_ttl) = dispose_ttl {
                    self.schedule_page_instance_dispose_timer(id, dispose_ttl)?;
                } else {
                    self.cancel_page_instance_dispose_timer(id);
                }
            }
            PageInstanceEvent::Disposed { reason } => {
                self.cancel_page_instance_dispose_timer(id);
                self.dispose_page_instance(id, reason)?;
            }
            PageInstanceEvent::Resized { .. } => {}
        }

        Ok(())
    }

    pub fn dispose_page_instance(
        &self,
        id: &PageInstanceId,
        reason: CloseReason,
    ) -> Result<(), LxAppError> {
        self.dispose_page_instance_internal(id, reason, true)
    }

    pub(super) fn dispose_page_instance_internal(
        &self,
        id: &PageInstanceId,
        reason: CloseReason,
        dispatch_on_hide: bool,
    ) -> Result<(), LxAppError> {
        self.cancel_page_instance_dispose_timer(id);
        self.cancel_page_reset(id.as_str());
        let child_reason = if matches!(reason, CloseReason::AppClosed) {
            CloseReason::AppClosed
        } else {
            CloseReason::OwnerClosed
        };
        self.close_surfaces_for_owner(id, child_reason);

        // If this page IS the content of a surface (i.e. it lives inside an
        // overlay the owner opened), close that surface too so the owner's
        // `Surface` handle gets an onClose. Without this, an SDK-side reclaim
        // disposes the page silently and the owner keeps postMessaging into
        // a dead handle. Propagate the actual reason (e.g. Reclaimed) so JS
        // can distinguish SDK-initiated cleanup from a user close.
        self.close_surfaces_hosting(id, reason);

        let page = self.get_page_by_instance_id(id).ok_or_else(|| {
            LxAppError::ResourceNotFound(format!("page instance id: {}", id.as_str()))
        })?;
        let path = page.path();

        if dispatch_on_hide {
            page.dispatch_lifecycle_event(crate::lifecycle::PageLifecycleEvent::OnHide);
        }
        page.dispatch_lifecycle_event(crate::lifecycle::PageLifecycleEvent::OnUnload);
        page.detach_webview();

        crate::view_call::cancel_view_calls_for_page_instances(
            &[id.to_string()],
            "PageInstance disposed while waiting for view response",
        );

        if let Ok(mut state) = self.state.lock() {
            state.pages_by_id.lock().unwrap().remove(id.as_str());
            state
                .page_instance_runtime
                .lock()
                .unwrap()
                .remove(id.as_str());
            state
                .page_stack
                .lock()
                .unwrap()
                .retain(|entry| entry != id.as_str());
            if let Ok(mut pins) = state.path_pins.lock() {
                pins.retain(|_, pinned| pinned != id.as_str());
            }
            state.page_chrome_layouts.remove(id.as_str());
        }

        destroy_webview(&page.webtag());

        if let Err(e) =
            self.executor
                .terminate_page_svc(self.clone_arc(), path.clone(), Some(id.to_string()))
        {
            warn!(
                "Failed to terminate page service while disposing instance {}: {}",
                id, e
            )
            .with_appid(self.appid.clone())
            .with_path(path.clone());
        }

        info!("Disposed page instance {} reason={}", id, reason.as_str())
            .with_appid(self.appid.clone())
            .with_path(path);

        Ok(())
    }

    /// Reject a navigation whose stack rules would fail it, BEFORE the entry
    /// mutates anything — cached query, opener bindings. Mirrors the checks
    /// in navigate_to_internal, which stay as the backstop.
    pub fn validate_navigation_entry(
        &self,
        url: &str,
        nav_type: crate::page::NavigationType,
    ) -> Result<(), LxAppError> {
        let resolved = match crate::route::resolve_route(self, url) {
            Ok(resolved) => resolved,
            Err(_) => return Ok(()),
        };
        let path = resolved.internal_path();
        let pinned = self
            .pinned_page(&path)
            .map(|page| page.instance_id_string());
        validate_navigation_stack(&self.get_page_stack(), pinned.as_deref(), &path, nav_type)
    }

    /// Canonical route path a URL resolves to (query stripped).
    pub fn resolve_entry_path(&self, url: &str) -> String {
        self.resolve_entry_route(url).internal_path().to_string()
    }

    pub(crate) fn resolve_entry_route(&self, url: &str) -> crate::route::ResolvedRoute {
        crate::route::resolve_route(self, url).unwrap_or_else(|e| {
            error!("Failed to resolve page url '{}': {}", url, e).with_appid(self.appid.clone());
            let (path, query) = crate::startup::split_path_query(url);
            crate::route::ResolvedRoute {
                original: url.to_string(),
                query,
                target: crate::route::RouteTarget::Normal { path },
            }
        })
    }

    /// Build a fresh, unregistered PageInstance for the path. PageSvc creation
    /// + HTML load are handled inside PageInstance::new once WebView is ready.
    fn mint_page_instance(&self, path: &str) -> PageInstance {
        let appid = self.appid.clone();
        let lxapp_arc = self.clone_arc();
        PageInstance::new(appid, path.to_string(), self, move |page| {
            let lxapp_arc = lxapp_arc.clone();
            let page_clone = page.clone();
            async move {
                let result = async {
                    // Ensure PageSvc exists before loading HTML (for both regular and plugin pages)
                    let (ack_tx, ack_rx) = oneshot::channel::<Result<(), String>>();
                    lxapp_arc
                        .executor
                        .create_page_svc_with_ack(
                            lxapp_arc.clone(),
                            page_clone.path(),
                            Some(page_clone.instance_id_string()),
                            ack_tx,
                        )
                        .map_err(|e| e.to_string())?;

                    ack_rx
                        .await
                        .map_err(|e| {
                            format!("PageInstance service creation channel closed: {}", e)
                        })?
                        .map_err(|e| format!("PageInstance service creation failed: {}", e))?;

                    page_clone
                        .load_html()
                        .map_err(|e| format!("Failed to load HTML for page: {}", e))
                }
                .await;
                if result.is_err() {
                    lxapp_arc.remove_failed_page(&page_clone);
                }
                result
            }
        })
    }

    /// Pin tab pages as path singletons: switchTab returns to the warm
    /// instance, so it must stay resolvable while off the stack.
    fn pin_if_tabbar_page(&self, page: &PageInstance, path: &str) {
        if self
            .get_tabbar()
            .is_some_and(|tabbar| tabbar.is_tabbar_page(path))
        {
            self.pin_page_path(page);
        }
    }

    /// Get existing page or create a new one. Resolution never mints a
    /// duplicate for a route that is already live — navigation entry points
    /// use `create_page_for_entry` for that.
    pub fn get_or_create_page(&self, url: &str) -> PageInstance {
        let resolved = self.resolve_entry_route(url);
        let path = resolved.internal_path();
        // A cached instance serves every URL on its route. Every requested URL
        // must therefore replace the cached query, including the empty query;
        // if it only updates on `Some`, a later `/page` navigation incorrectly
        // inherits state from an earlier `/page?mode=...` visit.
        let query = resolved.query.unwrap_or_default();

        if let Some(page) = self.get_page(&path) {
            page.set_query(query);
            return page;
        }

        let candidate = self.mint_page_instance(&path);

        // Double-checked under the state lock: a concurrent navigation may
        // have created this route's instance while the candidate was built.
        let page = {
            let state = self.state.lock().unwrap();
            let mut pages_by_id = state.pages_by_id.lock().unwrap();
            let existing = pages_by_id
                .values()
                .find(|page| !page.is_isolated() && page.path() == path)
                .cloned();
            if let Some(page) = existing {
                page
            } else {
                pages_by_id.insert(candidate.instance_id_string(), candidate.clone());
                candidate
            }
        };

        self.pin_if_tabbar_page(&page, &path);
        self.evict_inactive_pages_if_needed();
        page.set_query(query);
        page
    }

    /// Resolve the instance a navigation entry lands on. Unlike
    /// `get_or_create_page`, a route whose instances are all on the stack gets
    /// a fresh instance — two stack entries never share one — which is what
    /// lets the same route appear on the stack twice.
    pub fn create_page_for_entry(&self, url: &str) -> PageInstance {
        let resolved = self.resolve_entry_route(url);
        let path = resolved.internal_path();
        let query = resolved.query.unwrap_or_default();

        // Path-pinned singletons (tab pages, headless services) always
        // re-enter their warm instance.
        if let Some(page) = self.pinned_page(&path) {
            page.set_query(query);
            return page;
        }

        // A parked off-stack instance is the warm re-entry path: adopt the
        // most recently active one instead of cold-creating a WebView.
        if let Some(page) = self.most_recent_off_stack_page(&path) {
            page.set_query(query);
            return page;
        }

        let page = self.mint_page_instance(&path);
        {
            let state = self.state.lock().unwrap();
            state
                .pages_by_id
                .lock()
                .unwrap()
                .insert(page.instance_id_string(), page.clone());
        }
        self.pin_if_tabbar_page(&page, &path);
        self.evict_inactive_pages_if_needed();
        page.set_query(query);
        page
    }

    /// Check if we need to evict pages before creating new ones
    /// Evict when page count exceeds: tabbar_items + PAGE_STACK_MAX
    fn should_evict_pages(&self) -> bool {
        let state = self.state.lock().unwrap();
        // Isolated surface pages have their own lifecycle and budget.
        let page_count = state
            .pages_by_id
            .lock()
            .unwrap()
            .values()
            .filter(|page| !page.is_isolated())
            .count();

        let max_allowed = if let Some(ref tabbar) = state.tabbar {
            tabbar.items.len() + PAGE_STACK_MAX
        } else {
            PAGE_STACK_MAX
        };

        page_count > max_allowed
    }

    /// Evict least recently used pages when memory is full
    fn evict_inactive_pages_if_needed(&self) {
        if !self.should_evict_pages() {
            return;
        }

        let state = self.state.lock().unwrap();
        let mut pages_by_id = state.pages_by_id.lock().unwrap();

        let protected_ids: std::collections::HashSet<String> = {
            let mut protected = state
                .page_stack
                .lock()
                .unwrap()
                .iter()
                .cloned()
                .collect::<std::collections::HashSet<_>>();
            if let Ok(pins) = state.path_pins.lock() {
                protected.extend(pins.values().cloned());
            }
            protected
        };

        let mut oldest_time: Option<Instant> = None;
        let mut oldest_id: Option<String> = None;

        for (id, page) in pages_by_id.iter() {
            // Isolated surface pages have their own dispose lifecycle.
            if page.is_isolated() || protected_ids.contains(id) {
                continue;
            }
            if let Some(last_active) = page.get_last_active_time()
                && oldest_time.is_none_or(|old| last_active < old)
            {
                oldest_time = Some(last_active);
                oldest_id = Some(id.clone());
            }
        }

        if let Some(id) = oldest_id
            && let Some(removed_page) = pages_by_id.remove(&id)
        {
            removed_page.cancel_bridge_work();
            let _ = self
                .executor
                .terminate_page_svc(self.clone_arc(), removed_page.path(), Some(id.clone()))
                .map_err(|e| {
                    warn!("Failed to request page termination: {}", e)
                        .with_appid(self.appid.clone())
                        .with_path(removed_page.path())
                });
            if let Some(cancel) = state
                .page_instance_dispose_timers
                .lock()
                .unwrap()
                .remove(id.as_str())
            {
                let _ = cancel.send(());
            }
            crate::view_call::cancel_view_calls_for_page_instances(
                std::slice::from_ref(&id),
                "PageInstance evicted while waiting for view response",
            );
            state
                .page_instance_runtime
                .lock()
                .unwrap()
                .remove(id.as_str());
            destroy_webview(&removed_page.webtag());
            info!("Evicted inactive page: {}", removed_page.path()).with_appid(self.appid.clone());
        }
    }

    /// Clear the page navigation stack
    /// This removes all pages from the navigation history
    pub(crate) fn clear_page_stack(&self) -> Result<(), LxAppError> {
        let state = self.state.lock().unwrap();
        state.page_stack.lock().unwrap().clear();
        Ok(())
    }

    /// Push a page instance onto the navigation stack.
    pub(crate) fn push_to_page_stack(&self, page: &PageInstance) -> Result<(), LxAppError> {
        let state = self.state.lock().unwrap();
        let mut stack = state.page_stack.lock().unwrap();

        // Navigation preflight normally catches this before any page state is
        // mutated. Keep the stack primitive strict as a final invariant guard.
        if stack.len() >= PAGE_STACK_MAX {
            return Err(LxAppError::ResourceExhausted(format!(
                "Page stack is full (capacity: {PAGE_STACK_MAX})"
            )));
        }

        // Add to the back of the stack (most recent)
        stack.push_back(page.instance_id_string());

        Ok(())
    }

    /// Remove the most recent entry from the navigation stack and return its
    /// instance, when it is still alive.
    pub(crate) fn pop_from_page_stack(&self) -> Option<PageInstance> {
        let state = self.state.lock().unwrap();
        let id = state.page_stack.lock().unwrap().pop_back()?;
        state.pages_by_id.lock().unwrap().get(&id).cloned()
    }

    /// Remove specific page instances and terminate their PageSvc.
    pub fn remove_pages(&self, instance_ids: &[String]) {
        crate::view_call::cancel_view_calls_for_page_instances(
            instance_ids,
            "PageInstance removed while waiting for view response",
        );

        if let Ok(state) = self.state.lock() {
            // Drop the ids from the stack in the same critical section. A
            // caller that reads the stack head while it still names a removed
            // instance cannot resolve it, and the callers here only clear the
            // stack afterwards, in a lock acquisition of their own.
            state
                .page_stack
                .lock()
                .unwrap()
                .retain(|entry| !instance_ids.iter().any(|id| id == entry));
            let mut pages_by_id = state.pages_by_id.lock().unwrap();
            for id in instance_ids {
                if let Some(page) = pages_by_id.remove(id) {
                    let _ = self
                        .executor
                        .terminate_page_svc(self.clone_arc(), page.path(), Some(id.clone()))
                        .map_err(|e| {
                            warn!("Failed to request page termination: {}", e)
                                .with_appid(self.appid.clone())
                                .with_path(page.path())
                        });
                    if let Some(cancel) = state
                        .page_instance_dispose_timers
                        .lock()
                        .unwrap()
                        .remove(id.as_str())
                    {
                        let _ = cancel.send(());
                    }
                    state
                        .page_instance_runtime
                        .lock()
                        .unwrap()
                        .remove(id.as_str());
                    if let Ok(mut pins) = state.path_pins.lock() {
                        pins.retain(|_, pinned| pinned != id);
                    }
                }
            }
        }
    }

    /// Get the current page stack size
    pub(crate) fn get_page_stack_size(&self) -> usize {
        self.state.lock().unwrap().page_stack.lock().unwrap().len()
    }

    /// Instance ids on the navigation stack, oldest → newest.
    pub fn get_page_stack(&self) -> Vec<String> {
        self.state
            .lock()
            .unwrap()
            .page_stack
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect()
    }

    /// Every live page instance, including surface-owned pages that are not on
    /// the navigation stack. App-wide state has to reach all of them.
    pub fn live_page_instances(&self) -> Vec<PageInstance> {
        let Ok(state) = self.state.lock() else {
            return Vec::new();
        };
        let Ok(pages) = state.pages_by_id.lock() else {
            return Vec::new();
        };
        pages.values().cloned().collect()
    }

    /// Live instances on the navigation stack, oldest → newest.
    pub fn get_page_stack_pages(&self) -> Vec<PageInstance> {
        let state = self.state.lock().unwrap();
        let pages_by_id = state.pages_by_id.lock().unwrap();
        state
            .page_stack
            .lock()
            .unwrap()
            .iter()
            .filter_map(|id| pages_by_id.get(id).cloned())
            .collect()
    }

    /// Peek at the current page's instance id without removing it.
    /// Route path of the current page (stack top), when non-empty. The stack
    /// itself stores instance ids; every external caller wants the route.
    pub fn peek_current_page(&self) -> Option<String> {
        self.peek_current_page_path()
    }

    /// Route path of the current page, when the stack is non-empty.
    pub fn peek_current_page_path(&self) -> Option<String> {
        self.current_page().ok().map(|page| page.path())
    }

    /// Route paths on the navigation stack, oldest → newest.
    pub fn get_page_stack_paths(&self) -> Vec<String> {
        self.get_page_stack_pages()
            .iter()
            .map(|page| page.path())
            .collect()
    }

    /// Return the current visible page or an error when the page stack is empty.
    ///
    /// The stack head and the instance map are read under one lock: reading
    /// them separately let a concurrent teardown land in between, so a caller
    /// saw an id the map had already dropped.
    pub fn current_page(&self) -> Result<PageInstance, LxAppError> {
        let state = self.state.lock().unwrap();
        let id = state
            .page_stack
            .lock()
            .unwrap()
            .back()
            .cloned()
            .ok_or_else(|| LxAppError::WebView("No current page".to_string()))?;
        state
            .pages_by_id
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| LxAppError::WebView(format!("Current page instance not found: {id}")))
    }

    /// Return a page by path or an error when that page is not currently alive.
    pub fn require_page(&self, path: &str) -> Result<PageInstance, LxAppError> {
        self.get_page(path)
            .ok_or_else(|| LxAppError::WebView(format!("PageInstance not found: {}", path)))
    }

    /// Snapshot every live page instance, including isolated surface pages.
    pub fn page_instance_runtime_info(&self) -> Vec<PageInstanceRuntimeInfo> {
        let (pages, records, stack) = match self.state.lock() {
            Ok(state) => {
                let pages = state
                    .pages_by_id
                    .lock()
                    .map(|pages| pages.values().cloned().collect::<Vec<_>>())
                    .unwrap_or_default();
                let records = state
                    .page_instance_runtime
                    .lock()
                    .map(|records| records.clone())
                    .unwrap_or_default();
                let stack = state
                    .page_stack
                    .lock()
                    .map(|stack| stack.iter().cloned().collect::<Vec<_>>())
                    .unwrap_or_default();
                (pages, records, stack)
            }
            Err(_) => return Vec::new(),
        };

        let stack_instances = stack
            .iter()
            .enumerate()
            .map(|(index, instance_id)| (instance_id.clone(), index))
            .collect::<HashMap<_, _>>();
        let current_id = stack_instances
            .iter()
            .max_by_key(|(_, index)| *index)
            .map(|(id, _)| id.clone());

        let mut infos = pages
            .into_iter()
            .map(|page| {
                let instance_id = page.instance_id_string();
                let record = records.get(&instance_id);
                let state = page.automation_state();
                let current = current_id.as_deref() == Some(instance_id.as_str());
                PageInstanceRuntimeInfo {
                    instance_id: instance_id.clone(),
                    name: record
                        .and_then(|record| record.page.definition.name.clone())
                        .or_else(|| self.page_definition_for_resolved_path(&page.path()).name),
                    path: page.path(),
                    query: state.query.clone(),
                    owner: record
                        .map(|record| record.owner.clone())
                        .unwrap_or(PageOwner::Host),
                    presentation: record
                        .map(|record| record.surface)
                        .unwrap_or(PresentationKind::Window),
                    lifecycle: effective_page_instance_lifecycle(
                        record.map(|record| record.lifecycle),
                        current,
                        state.lifecycle,
                        state.webview_attached,
                    )
                    .to_string(),
                    stack_index: stack_instances.get(&instance_id).copied(),
                    current,
                    state,
                }
            })
            .collect::<Vec<_>>();
        infos.sort_by(|left, right| {
            left.stack_index
                .is_none()
                .cmp(&right.stack_index.is_none())
                .then_with(|| left.stack_index.cmp(&right.stack_index))
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| left.instance_id.cmp(&right.instance_id))
        });
        infos
    }

    /// Call the current page View method without a payload and deserialize the response.
    pub async fn call_view<R>(&self, method: &str) -> Result<R, LxAppError>
    where
        R: DeserializeOwned,
    {
        self.current_page()?.call_view(method).await
    }

    /// Call the current page View method without a payload using explicit call options.
    pub async fn call_view_in<R>(
        &self,
        method: &str,
        options: ViewCallOptions,
    ) -> Result<R, LxAppError>
    where
        R: DeserializeOwned,
    {
        self.current_page()?.call_view_in(method, options).await
    }

    /// Call the current page View method with a typed payload and deserialize the response.
    pub async fn call_view_with<P, R>(&self, method: &str, params: &P) -> Result<R, LxAppError>
    where
        P: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        self.current_page()?.call_view_with(method, params).await
    }

    /// Call the current page View method with explicit call options.
    pub async fn call_view_with_in<P, R>(
        &self,
        method: &str,
        params: &P,
        options: ViewCallOptions,
    ) -> Result<R, LxAppError>
    where
        P: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        self.current_page()?
            .call_view_with_in(method, params, options)
            .await
    }

    /// Notify the AppService (logic.js layer) with a built-in event and optional JSON payload.
    pub fn appservice_notify(
        &self,
        event: AppServiceEvent,
        payload_json: Option<String>,
    ) -> Result<(), LxAppError> {
        if !self.logic_enabled() {
            return Ok(());
        }
        self.executor
            .call_app_service_event(self.clone_arc(), event, payload_json)
    }
}

fn isolated_page_waits_for_page_service(logic_enabled: bool) -> bool {
    // Shape C has no AppService or PageSvc. Its surface-owned View must load as
    // soon as the WebView exists instead of waiting on a service that cannot be
    // created.
    logic_enabled
}

fn effective_page_instance_lifecycle(
    recorded: Option<PageInstanceLifecycleState>,
    current: bool,
    page_lifecycle: &str,
    webview_attached: bool,
) -> &'static str {
    if current {
        return PageInstanceLifecycleState::Visible.as_str();
    }
    if recorded == Some(PageInstanceLifecycleState::Hidden) || page_lifecycle == "onHide" {
        return PageInstanceLifecycleState::Hidden.as_str();
    }
    match recorded {
        Some(PageInstanceLifecycleState::Mounted) => PageInstanceLifecycleState::Mounted.as_str(),
        Some(PageInstanceLifecycleState::Visible) => PageInstanceLifecycleState::Visible.as_str(),
        Some(PageInstanceLifecycleState::Disposed) => PageInstanceLifecycleState::Disposed.as_str(),
        Some(PageInstanceLifecycleState::Created) | None if webview_attached => {
            PageInstanceLifecycleState::Mounted.as_str()
        }
        Some(PageInstanceLifecycleState::Created) | None => {
            PageInstanceLifecycleState::Created.as_str()
        }
        Some(PageInstanceLifecycleState::Hidden) => unreachable!("handled above"),
    }
}

fn normalize_page_path(path: &str) -> &str {
    path.trim_start_matches('/')
}

/// Strip view extensions from path for comparison
fn strip_extension(path: &str) -> &str {
    for ext in [".tsx", ".jsx", ".vue"] {
        if let Some(p) = path.strip_suffix(ext) {
            return p;
        }
    }
    path
}

/// Find matching page in config, return with extension
fn find_matching_page_path<'a>(pages: &'a [String], path: &str) -> Option<&'a str> {
    let path = normalize_page_path(path);
    let path_no_ext = strip_extension(path);
    pages
        .iter()
        .find(|p| {
            let p = normalize_page_path(p);
            p == path || strip_extension(p) == path_no_ext
        })
        .map(|s| s.as_str())
}

fn extract_plugin_page_path(url: &str) -> Option<String> {
    let (path, _) = crate::startup::split_path_query(url);
    crate::plugin::parse_plugin_url(&path)
        .or_else(|| crate::plugin::parse_plugin_page_path(&path))
        .map(|(_, page_path)| page_path)
}

fn plugin_page_map_contains(
    pages: &std::collections::BTreeMap<String, String>,
    requested_path: &str,
    resolved_path: &str,
) -> bool {
    let requested = normalize_page_path(requested_path);
    let resolved = normalize_page_path(resolved_path);
    pages.iter().any(|(key, value)| {
        let key = normalize_page_path(key);
        let value = normalize_page_path(value);
        key == requested || value == requested || key == resolved || value == resolved
    })
}

#[cfg(test)]
mod tests {
    use super::{
        PAGE_STACK_MAX, PageInstanceLifecycleState, effective_page_instance_lifecycle,
        find_matching_page_path, isolated_page_waits_for_page_service, validate_navigation_stack,
    };
    use crate::NavigationType;

    #[test]
    fn automation_lifecycle_does_not_report_ready_pages_as_created() {
        assert_eq!(
            effective_page_instance_lifecycle(
                Some(PageInstanceLifecycleState::Created),
                true,
                "onReady",
                true,
            ),
            "visible"
        );
        assert_eq!(
            effective_page_instance_lifecycle(None, false, "onHide", true),
            "hidden"
        );
        assert_eq!(
            effective_page_instance_lifecycle(None, false, "onReady", true),
            "mounted"
        );
    }

    #[test]
    fn host_webview_source_extension_resolves_to_the_configured_page() {
        let pages = vec!["pages/home/index".to_string()];

        assert_eq!(
            find_matching_page_path(&pages, "pages/home/index.tsx"),
            Some("pages/home/index")
        );
    }

    #[test]
    fn logic_disabled_isolated_page_does_not_wait_for_page_service() {
        assert!(!isolated_page_waits_for_page_service(false));
        assert!(isolated_page_waits_for_page_service(true));
    }

    fn navigation_reason(error: crate::LxAppError) -> String {
        match error {
            crate::LxAppError::RongJSHost {
                data: Some(data), ..
            } => data["reason"].as_str().unwrap().to_string(),
            other => panic!("unexpected navigation error: {other:?}"),
        }
    }

    #[test]
    fn navigation_preflight_rejects_a_pinned_singleton_already_on_the_stack() {
        let stack = vec!["home-instance".to_string(), "detail-instance".to_string()];

        let error = validate_navigation_stack(
            &stack,
            Some("home-instance"),
            "pages/home",
            NavigationType::Forward,
        )
        .unwrap_err();
        assert_eq!(navigation_reason(error), "duplicate_route");
    }

    #[test]
    fn navigation_preflight_allows_duplicate_routes_for_fresh_instances() {
        let stack = vec!["detail-1".to_string(), "detail-2".to_string()];

        validate_navigation_stack(&stack, None, "pages/detail", NavigationType::Forward).unwrap();
        validate_navigation_stack(&stack, None, "pages/detail", NavigationType::Replace).unwrap();
    }

    #[test]
    fn navigation_preflight_rejects_a_full_stack() {
        let stack = (0..PAGE_STACK_MAX)
            .map(|index| format!("instance-{index}"))
            .collect::<Vec<_>>();

        let error = validate_navigation_stack(&stack, None, "pages/new", NavigationType::Forward)
            .unwrap_err();
        assert_eq!(navigation_reason(error), "stack_full");
    }
}
