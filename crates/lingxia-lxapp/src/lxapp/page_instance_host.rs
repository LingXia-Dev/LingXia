//! Host-side page instance orchestration for [`LxApp`].
//!
//! This module owns route resolution, page-instance lifecycle integration,
//! page stack operations, and view-call convenience APIs.

use super::*;

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

        let config = if self.logic_enabled() {
            PageConfig::from_json(self, &config_path)
        } else {
            PageConfig::default()
        };

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
        let page = PageInstance::new_with_webtag_instance(
            appid.clone(),
            path.to_string(),
            self,
            Some(WebTagInstance::PageInstanceId),
            move |page| {
                let lxapp_arc = lxapp_arc.clone();
                let page_clone = page.clone();
                async move {
                    let (ack_tx, ack_rx) = oneshot::channel::<Result<(), String>>();
                    if let Err(e) = lxapp_arc.executor.create_page_svc_with_ack(
                        lxapp_arc.clone(),
                        page_clone.path(),
                        Some(page_clone.instance_id_string()),
                        ack_tx,
                    ) {
                        return Err(e.to_string());
                    }

                    ack_rx
                        .await
                        .map_err(|e| {
                            format!("PageInstance service creation channel closed: {}", e)
                        })?
                        .map_err(|e| format!("PageInstance service creation failed: {}", e))?;

                    page_clone
                        .load_html()
                        .map_err(|e| format!("Failed to load HTML for page: {}", e))?;
                    lxapp_arc
                        .notify_page_instance(&page_clone.instance_id(), PageInstanceEvent::Mounted)
                        .map_err(|e| format!("Failed to mount page instance: {}", e))
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

        if let Ok(state) = self.state.lock() {
            let mut pages = state.pages.lock().unwrap();
            let canonical_instance_id = pages
                .get(&path)
                .map(|existing| existing.instance_id_string());
            let remove_stack_path =
                disposed_instance_owns_stack_path(canonical_instance_id.as_deref(), id.as_str());
            if remove_stack_path {
                pages.remove(&path);
            }
            state.pages_by_id.lock().unwrap().remove(id.as_str());
            state
                .page_instance_runtime
                .lock()
                .unwrap()
                .remove(id.as_str());
            if remove_stack_path {
                state
                    .page_stack
                    .lock()
                    .unwrap()
                    .retain(|stack_path| stack_path != &path);
            }
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

    /// Get existing page or create a new one.
    /// PageSvc creation + HTML load are handled inside PageInstance::new once WebView is ready.
    pub fn get_or_create_page(&self, url: &str) -> PageInstance {
        let resolved = crate::route::resolve_route(self, url).unwrap_or_else(|e| {
            error!("Failed to resolve page url '{}': {}", url, e).with_appid(self.appid.clone());
            let (path, query) = crate::startup::split_path_query(url);
            crate::route::ResolvedRoute {
                original: url.to_string(),
                query,
                target: crate::route::RouteTarget::Normal { path },
            }
        });

        let path = resolved.internal_path();
        let query = resolved.query;

        let _creation_guard = self.page_creation_lock.lock().unwrap();
        if let Some(page) = self.get_page(&path) {
            if let Some(query) = query.clone() {
                page.set_query(query);
            }
            return page;
        }

        let appid = self.appid.clone();
        let lxapp_arc = self.clone_arc();
        let candidate = PageInstance::new(appid.clone(), path.to_string(), self, move |page| {
            let lxapp_arc = lxapp_arc.clone();
            let page_clone = page.clone();
            async move {
                // Ensure PageSvc exists before loading HTML (for both regular and plugin pages)
                let (ack_tx, ack_rx) = oneshot::channel::<Result<(), String>>();
                if let Err(e) = lxapp_arc.executor.create_page_svc_with_ack(
                    lxapp_arc.clone(),
                    page_clone.path(),
                    None,
                    ack_tx,
                ) {
                    return Err(e.to_string());
                }

                ack_rx
                    .await
                    .map_err(|e| format!("PageInstance service creation channel closed: {}", e))?
                    .map_err(|e| format!("PageInstance service creation failed: {}", e))?;

                page_clone
                    .load_html()
                    .map_err(|e| format!("Failed to load HTML for page: {}", e))
            }
        });

        let page = {
            let state = self.state.lock().unwrap();
            let mut pages = state.pages.lock().unwrap();

            if let Some(page) = pages.get(&path) {
                page.clone()
            } else {
                state
                    .pages_by_id
                    .lock()
                    .unwrap()
                    .insert(candidate.instance_id_string(), candidate.clone());
                pages.insert(path.clone(), candidate.clone());
                candidate
            }
        };
        drop(_creation_guard);

        self.evict_inactive_pages_if_needed();

        if let Some(query) = query {
            page.set_query(query);
        }

        page
    }

    /// Check if we need to evict pages before creating new ones
    /// Evict when page count exceeds: tabbar_items + PAGE_STACK_MAX
    fn should_evict_pages(&self) -> bool {
        let state = self.state.lock().unwrap();
        let page_count = state.pages.lock().unwrap().len();

        let max_allowed = if let Some(ref tabbar) = state.tabbar {
            tabbar.list.len() + PAGE_STACK_MAX
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
        let mut pages = state.pages.lock().unwrap();

        // Find the least recently used page (excluding current page in stack)
        let current_page = state.page_stack.lock().unwrap().back().cloned();

        let mut oldest_time: Option<Instant> = None;
        let mut oldest_path: Option<String> = None;
        let mut oldest_page_instance_id: Option<String> = None;

        for (path, page) in pages.iter() {
            if Some(path) == current_page.as_ref() {
                continue; // Don't evict current page
            }

            // Don't evict tabbar pages
            if page.is_tabbar_page() {
                info!("Skipping tabbar page for eviction: {}", path).with_appid(self.appid.clone());
                continue;
            }

            if let Some(last_active) = page.get_last_active_time()
                && oldest_time.is_none_or(|old| last_active < old)
            {
                oldest_time = Some(last_active);
                oldest_path = Some(path.clone());
                oldest_page_instance_id = Some(page.instance_id_string());
            }
        }

        // Remove the oldest page
        if let Some(path) = oldest_path.clone() {
            if let Some(page) = pages.get(&path) {
                page.cancel_pending_view_requests();
            }
            // First, ask AppService to remove the PageSvc for this path (object-identity safe)
            let _ = self
                .executor
                .terminate_page_svc(
                    self.clone_arc(),
                    path.clone(),
                    oldest_page_instance_id.clone(),
                )
                .map_err(|e| {
                    warn!("Failed to request page termination: {}", e)
                        .with_appid(self.appid.clone())
                        .with_path(path.clone())
                });

            // Then remove from native registry
            if let Some(removed_page) = pages.remove(&path) {
                if let Some(cancel) = state
                    .page_instance_dispose_timers
                    .lock()
                    .unwrap()
                    .remove(removed_page.instance_id().as_str())
                {
                    let _ = cancel.send(());
                }
                crate::view_call::cancel_view_calls_for_page_instances(
                    &[removed_page.instance_id_string()],
                    "PageInstance evicted while waiting for view response",
                );
                state
                    .pages_by_id
                    .lock()
                    .unwrap()
                    .remove(removed_page.instance_id().as_str());
                state
                    .page_instance_runtime
                    .lock()
                    .unwrap()
                    .remove(removed_page.instance_id().as_str());
                destroy_webview(&removed_page.webtag());
                info!("Evicted inactive page: {}", path).with_appid(self.appid.clone());
            } else {
                warn!("Failed to evict page (not found): {}", path).with_appid(self.appid.clone());
            }
        }
    }

    /// Check if the page stack is considered full
    /// Returns true when stack size reaches PAGE_STACK_MAX
    pub(crate) fn is_page_stack_full(&self) -> bool {
        self.get_page_stack_size() >= PAGE_STACK_MAX
    }

    /// Clear the page navigation stack
    /// This removes all pages from the navigation history
    pub(crate) fn clear_page_stack(&self) -> Result<(), LxAppError> {
        let state = self.state.lock().unwrap();
        state.page_stack.lock().unwrap().clear();
        Ok(())
    }

    /// Add a page to the navigation stack.
    pub(crate) fn push_to_page_stack(&self, path: &str) -> Result<(), LxAppError> {
        let state = self.state.lock().unwrap();
        let mut stack = state.page_stack.lock().unwrap();

        // If stack is full, do nothing
        if stack.len() >= PAGE_STACK_MAX {
            return Ok(());
        }

        // Add to the back of the stack (most recent)
        stack.push_back(path.to_string());

        Ok(())
    }

    /// Remove the most recent page from the navigation stack
    /// Returns the path of the removed page, or None if stack is empty
    pub(crate) fn pop_from_page_stack(&self) -> Option<String> {
        let state = self.state.lock().unwrap();
        state.page_stack.lock().unwrap().pop_back()
    }

    /// Remove specific pages from the page map and terminate their PageSvc.
    pub fn remove_pages(&self, paths: &[String]) {
        let page_instances = {
            let state = self.state.lock().unwrap();
            let pages = state.pages.lock().unwrap();
            paths
                .iter()
                .filter_map(|path| {
                    pages
                        .get(path)
                        .map(|page| (path.clone(), page.instance_id_string()))
                })
                .collect::<Vec<_>>()
        };
        let page_instance_ids = page_instances
            .iter()
            .map(|(_, id)| id.clone())
            .collect::<Vec<_>>();
        crate::view_call::cancel_view_calls_for_page_instances(
            &page_instance_ids,
            "PageInstance removed while waiting for view response",
        );

        let lxapp = self.clone_arc();
        for (path, page_instance_id) in &page_instances {
            let _ = self
                .executor
                .terminate_page_svc(lxapp.clone(), path.clone(), Some(page_instance_id.clone()))
                .map_err(|e| {
                    warn!("Failed to request page termination: {}", e)
                        .with_appid(self.appid.clone())
                        .with_path(path.clone())
                });
        }

        if let Ok(state) = self.state.lock() {
            let mut pages = state.pages.lock().unwrap();
            for path in paths {
                if let Some(page) = pages.remove(path) {
                    if let Some(cancel) = state
                        .page_instance_dispose_timers
                        .lock()
                        .unwrap()
                        .remove(page.instance_id().as_str())
                    {
                        let _ = cancel.send(());
                    }
                    state
                        .pages_by_id
                        .lock()
                        .unwrap()
                        .remove(page.instance_id().as_str());
                    state
                        .page_instance_runtime
                        .lock()
                        .unwrap()
                        .remove(page.instance_id().as_str());
                }
            }
        }
    }

    /// Get the current page stack size
    pub(crate) fn get_page_stack_size(&self) -> usize {
        self.state.lock().unwrap().page_stack.lock().unwrap().len()
    }

    /// Get a copy of the current page stack
    /// Returns a vector of page paths in stack order (oldest to newest)
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

    /// Peek at the current page path without removing it from the stack
    /// Returns None if the stack is empty
    pub fn peek_current_page(&self) -> Option<String> {
        self.state
            .lock()
            .unwrap()
            .page_stack
            .lock()
            .unwrap()
            .back()
            .cloned()
    }

    /// Return the current visible page or an error when the page stack is empty.
    pub fn current_page(&self) -> Result<PageInstance, LxAppError> {
        let path = self
            .peek_current_page()
            .ok_or_else(|| LxAppError::WebView("No current page".to_string()))?;
        self.require_page(&path)
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
            .filter_map(|path| self.get_page(path))
            .enumerate()
            .map(|(index, page)| (page.instance_id_string(), index))
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

fn disposed_instance_owns_stack_path(
    canonical_instance_id: Option<&str>,
    disposed_instance_id: &str,
) -> bool {
    canonical_instance_id == Some(disposed_instance_id)
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
        PageInstanceLifecycleState, disposed_instance_owns_stack_path,
        effective_page_instance_lifecycle,
    };

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
    fn disposing_isolated_surface_page_keeps_canonical_stack_path() {
        assert!(disposed_instance_owns_stack_path(
            Some("stack-instance"),
            "stack-instance"
        ));
        assert!(!disposed_instance_owns_stack_path(
            Some("stack-instance"),
            "surface-instance"
        ));
        assert!(!disposed_instance_owns_stack_path(None, "surface-instance"));
    }
}
