//! WebView2 event handler registration (navigation, downloads,
//! messages, resource requests).

use super::*;
use crate::events::normalizer::{self, NativeNavigationResult, NativeSignal};
use crate::traits::{ContextualSchemeRequest, LoadError, LoadErrorKind, SchemeRequestFrame};

/// Map a WebView2 `COREWEBVIEW2_WEB_ERROR_STATUS` to the normalized error
/// kind. Numeric values per the WebView2 SDK enum.
fn windows_load_error_kind(status: i32) -> LoadErrorKind {
    match status {
        1..=5 => LoadErrorKind::Security,
        6 | 9..=12 => LoadErrorKind::Network,
        7 => LoadErrorKind::Timeout,
        13 => LoadErrorKind::Dns,
        _ => LoadErrorKind::Unknown,
    }
}

fn windows_message_source(source: String) -> WebMessageSource {
    WebMessageSource::diagnostic_url(Some(source))
}

fn windows_web_message_frame(main_document_event: bool) -> WebMessageFrame {
    if main_document_event {
        WebMessageFrame::TopLevel
    } else {
        WebMessageFrame::Subframe
    }
}

fn windows_scheme_request_frame(_context: COREWEBVIEW2_WEB_RESOURCE_CONTEXT) -> SchemeRequestFrame {
    // WebResourceContext names a resource *type* only. In particular,
    // `Document` does not bind this request to the top-level frame, and the
    // callback exposes no frame identity or matching navigation proof. IFRAME
    // must therefore never be promoted, and neither may Document.
    SchemeRequestFrame::Unproven
}

fn native_callback_identity_matches(
    callback_native_view_id: NativeWebViewId,
    registered_native_view_id: NativeWebViewId,
) -> bool {
    callback_native_view_id == registered_native_view_id
}

/// Resolve a native callback only if its concrete WebView is still the one
/// registered for this logical tag. A tag can be reused while an old WebView2
/// controller is still draining callbacks.
fn current_native_callback_webview(
    webtag: &WebTag,
    native_view_id: NativeWebViewId,
) -> Option<Arc<crate::WebView>> {
    find_webview_by_native_view_id(webtag, native_view_id).filter(|webview| {
        native_callback_identity_matches(native_view_id, webview.native_view_id())
    })
}

pub(crate) fn register_event_handlers(
    env: &ICoreWebView2Environment,
    webview: &ICoreWebView2,
    webtag: WebTag,
    native_view_id: NativeWebViewId,
    registered_schemes: &[String],
    memory_pages: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    document_authority: Arc<document::WindowsDocumentAuthority>,
) -> StdResult<()> {
    let started_tag = webtag.clone();
    let started_native_view_id = native_view_id;
    let started_document_authority = Arc::clone(&document_authority);
    unsafe {
        let mut token = 0;
        webview
            .add_NavigationStarting(
                &NavigationStartingEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };

                    let mut uri = PWSTR::null();
                    args.Uri(&mut uri)?;
                    let uri = CoTaskMemPWSTR::from(uri).to_string();

                    let mut navigation_id = 0u64;
                    args.NavigationId(&mut navigation_id)?;

                    let trusted_start =
                        started_document_authority.navigation_start(&uri, navigation_id);

                    if let Some(webview) =
                        current_native_callback_webview(&started_tag, started_native_view_id)
                        && matches!(
                            webview.handle_navigation(&crate::NavigationRequest::new(
                                uri.clone(),
                                false,
                                true,
                            )),
                            NavigationPolicy::Cancel
                        )
                    {
                        if let document::TrustedNavigationStart::Attest { intent, .. }
                        | document::TrustedNavigationStart::Revoke(intent) = trusted_start
                        {
                            started_document_authority.revoke_if_matches(intent);
                            normalizer::revoke_trusted_load(
                                &started_tag,
                                started_native_view_id,
                                intent,
                            );
                        }
                        // Policy rejected before loading: the follow-up
                        // completion for this key is expected and consumed.
                        normalizer::submit(
                            &started_tag,
                            started_native_view_id,
                            NativeSignal::NavigationSuppressed {
                                key: Some(navigation_id),
                            },
                        );
                        args.SetCancel(true)?;
                        return Ok(());
                    }

                    match trusted_start {
                        document::TrustedNavigationStart::Attest {
                            intent,
                            navigation_key,
                        } => {
                            if !normalizer::attest_trusted_load(
                                &started_tag,
                                started_native_view_id,
                                intent,
                                navigation_key,
                            ) {
                                started_document_authority.revoke_if_matches(intent);
                                normalizer::revoke_trusted_load(
                                    &started_tag,
                                    started_native_view_id,
                                    intent,
                                );
                            }
                        }
                        document::TrustedNavigationStart::Revoke(intent) => {
                            normalizer::revoke_trusted_load(
                                &started_tag,
                                started_native_view_id,
                                intent,
                            );
                        }
                        document::TrustedNavigationStart::Untrusted => {}
                    }

                    // Redirect restarts reuse the native id; the tracker
                    // coalesces them into one attempt.
                    normalizer::submit(
                        &started_tag,
                        started_native_view_id,
                        NativeSignal::NavigationStarted {
                            key: Some(navigation_id),
                            url: uri,
                        },
                    );
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| {
                WebViewError::WebView(format!("add_NavigationStarting failed: {err}"))
            })?;
    }

    let frame_started_tag = webtag.clone();
    let frame_started_native_view_id = native_view_id;
    unsafe {
        let mut token = 0;
        webview
            .add_FrameNavigationStarting(
                &NavigationStartingEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };

                    let mut uri = PWSTR::null();
                    args.Uri(&mut uri)?;
                    let uri = CoTaskMemPWSTR::from(uri).to_string();
                    if let Some(webview) = current_native_callback_webview(
                        &frame_started_tag,
                        frame_started_native_view_id,
                    ) && matches!(
                        webview
                            .handle_navigation(&crate::NavigationRequest::new(uri, false, false,)),
                        NavigationPolicy::Cancel
                    ) {
                        args.SetCancel(true)?;
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| {
                WebViewError::WebView(format!("add_FrameNavigationStarting failed: {err}"))
            })?;
    }

    let committed_tag = webtag.clone();
    let committed_native_view_id = native_view_id;
    let committed_document_authority = Arc::clone(&document_authority);
    unsafe {
        let mut token = 0;
        webview
            .add_ContentLoading(
                &ContentLoadingEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    let mut navigation_id = 0u64;
                    args.NavigationId(&mut navigation_id)?;
                    committed_document_authority.navigation_finished(navigation_id);
                    // Commit evidence: the displayed document was replaced.
                    normalizer::submit(
                        &committed_tag,
                        committed_native_view_id,
                        NativeSignal::DocumentCommitted {
                            key: Some(navigation_id),
                        },
                    );
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| WebViewError::WebView(format!("add_ContentLoading failed: {err}")))?;
    }

    let finished_tag = webtag.clone();
    let finished_native_view_id = native_view_id;
    let finished_document_authority = Arc::clone(&document_authority);
    unsafe {
        let mut token = 0;
        webview
            .add_NavigationCompleted(
                &NavigationCompletedEventHandler::create(Box::new(move |sender, args| {
                    let (Some(sender), Some(args)) = (sender, args) else {
                        return Ok(());
                    };
                    let mut navigation_id = 0u64;
                    args.NavigationId(&mut navigation_id)?;
                    finished_document_authority.navigation_finished(navigation_id);
                    let mut succeeded = BOOL::default();
                    args.IsSuccess(&mut succeeded)?;
                    let result = if succeeded.as_bool() {
                        // Final URL captured inside the callback: WebView2 COM
                        // objects are thread-affine and cannot be queried later.
                        let mut source = PWSTR::null();
                        sender.Source(&mut source)?;
                        let final_url = CoTaskMemPWSTR::from(source).to_string();
                        NativeNavigationResult::Succeeded { final_url }
                    } else {
                        let mut status = COREWEBVIEW2_WEB_ERROR_STATUS::default();
                        args.WebErrorStatus(&mut status)?;
                        // 14 = OPERATION_CANCELED: control flow, not an error.
                        if status.0 == 14 {
                            NativeNavigationResult::Cancelled(None)
                        } else {
                            NativeNavigationResult::Failed(LoadError {
                                failing_url: None,
                                kind: windows_load_error_kind(status.0),
                                description: format!("WebView2 web error status {}", status.0),
                            })
                        }
                    };
                    normalizer::submit(
                        &finished_tag,
                        finished_native_view_id,
                        NativeSignal::NavigationFinished {
                            key: Some(navigation_id),
                            result,
                        },
                    );
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| {
                WebViewError::WebView(format!("add_NavigationCompleted failed: {err}"))
            })?;
    }

    let source_tag = webtag.clone();
    let source_native_view_id = native_view_id;
    unsafe {
        let mut token = 0;
        webview
            .add_SourceChanged(
                &SourceChangedEventHandler::create(Box::new(move |sender, _args| {
                    let Some(sender) = sender else {
                        return Ok(());
                    };
                    let mut source = PWSTR::null();
                    sender.Source(&mut source)?;
                    let source = CoTaskMemPWSTR::from(source).to_string();
                    normalizer::submit(
                        &source_tag,
                        source_native_view_id,
                        NativeSignal::LocationChanged { url: source },
                    );
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| WebViewError::WebView(format!("add_SourceChanged failed: {err}")))?;
    }

    let history_tag = webtag.clone();
    let history_native_view_id = native_view_id;
    unsafe {
        let mut token = 0;
        webview
            .add_HistoryChanged(
                &HistoryChangedEventHandler::create(Box::new(move |sender, _args| {
                    let Some(sender) = sender else {
                        return Ok(());
                    };
                    let mut can_back = BOOL::default();
                    let mut can_forward = BOOL::default();
                    sender.CanGoBack(&mut can_back)?;
                    sender.CanGoForward(&mut can_forward)?;
                    normalizer::submit(
                        &history_tag,
                        history_native_view_id,
                        NativeSignal::BackForwardChanged {
                            can_go_back: can_back.as_bool(),
                            can_go_forward: can_forward.as_bool(),
                        },
                    );
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| WebViewError::WebView(format!("add_HistoryChanged failed: {err}")))?;
    }

    let title_tag = webtag.clone();
    let title_native_view_id = native_view_id;
    unsafe {
        let mut token = 0;
        webview
            .add_DocumentTitleChanged(
                &DocumentTitleChangedEventHandler::create(Box::new(move |sender, _args| {
                    let Some(sender) = sender else {
                        return Ok(());
                    };
                    let mut title = PWSTR::null();
                    sender.DocumentTitle(&mut title)?;
                    let title = CoTaskMemPWSTR::from(title).to_string();
                    if !title.is_empty() {
                        normalizer::submit(
                            &title_tag,
                            title_native_view_id,
                            NativeSignal::TitleChanged { title: Some(title) },
                        );
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| {
                WebViewError::WebView(format!("add_DocumentTitleChanged failed: {err}"))
            })?;
    }

    // Favicon change notifications need ICoreWebView2_15 (newer WebView2
    // runtimes); older runtimes simply do without favicons.
    let favicon_tag = webtag.clone();
    let favicon_native_view_id = native_view_id;
    if let Ok(webview15) = webview.cast::<ICoreWebView2_15>() {
        let handler = FaviconChangedEventHandler::create(Box::new(move |sender, _args| {
            let Some(sender) = sender else {
                return Ok(());
            };
            let Ok(sender15) = sender.cast::<ICoreWebView2_15>() else {
                return Ok(());
            };
            let tag = favicon_tag.clone();
            unsafe {
                sender15.GetFavicon(
                    COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG,
                    &GetFaviconCompletedHandler::create(Box::new(move |result, stream| {
                        if result.is_err() {
                            return Ok(());
                        }
                        // No stream / empty bytes = page has no favicon.
                        let png_bytes = stream
                            .as_ref()
                            .and_then(|stream| read_stream_to_end(stream).ok())
                            .filter(|bytes| !bytes.is_empty());
                        normalizer::submit(
                            &tag,
                            favicon_native_view_id,
                            NativeSignal::FaviconChanged { png_bytes },
                        );
                        Ok(())
                    })),
                )?;
            }
            Ok(())
        }));
        let mut token = 0;
        if let Err(err) = unsafe { webview15.add_FaviconChanged(&handler, &mut token) } {
            // Favicons are cosmetic; never fail webview creation over them.
            log::warn!("add_FaviconChanged failed: {err}");
        }
    }

    let new_window_tag = webtag.clone();
    let new_window_native_view_id = native_view_id;
    unsafe {
        let mut token = 0;
        webview
            .add_NewWindowRequested(
                &NewWindowRequestedEventHandler::create(Box::new(move |sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };

                    let uri = take_request_string(|slot| args.Uri(slot))?;
                    let Some(webview) =
                        current_native_callback_webview(&new_window_tag, new_window_native_view_id)
                    else {
                        args.SetHandled(true)?;
                        return Ok(());
                    };

                    match webview.handle_new_window(&uri) {
                        NewWindowPolicy::LoadInSelf => {
                            if let Some(sender) = sender {
                                let uri = CoTaskMemPWSTR::from(uri.as_str());
                                sender.Navigate(*uri.as_ref().as_pcwstr())?;
                            }
                            args.SetHandled(true)?;
                        }
                        NewWindowPolicy::Cancel => {
                            args.SetHandled(true)?;
                        }
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| {
                WebViewError::WebView(format!("add_NewWindowRequested failed: {err}"))
            })?;
    }

    let download_tag = webtag.clone();
    let download_native_view_id = native_view_id;
    unsafe {
        let webview4: ICoreWebView2_4 = webview.cast().map_err(|err| {
            WebViewError::WebView(format!("WebView2_4 cast failed for downloads: {err}"))
        })?;
        let mut token = 0;
        webview4
            .add_DownloadStarting(
                &DownloadStartingEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    let Some(webview) =
                        current_native_callback_webview(&download_tag, download_native_view_id)
                    else {
                        return Ok(());
                    };
                    if !webview.has_download_handler() {
                        return Ok(());
                    }

                    let operation = args.DownloadOperation()?;
                    let request = download_request_from_operation(&operation)?;
                    webview.handle_download(request);
                    args.SetCancel(true)?;
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| WebViewError::WebView(format!("add_DownloadStarting failed: {err}")))?;
    }

    // The CoreWebView2-level event is the main document. Iframes have their
    // own Frame2 event; register it explicitly so frame messages retain a
    // fail-closed `Subframe` proof instead of being conflated with top-level.
    let frame_message_tag = webtag.clone();
    unsafe {
        let webview4: ICoreWebView2_4 = webview.cast().map_err(|err| {
            WebViewError::WebView(format!("WebView2_4 cast failed for frame messages: {err}"))
        })?;
        let mut token = 0;
        webview4
            .add_FrameCreated(
                &FrameCreatedEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    if current_native_callback_webview(&frame_message_tag, native_view_id).is_none()
                    {
                        return Ok(());
                    }
                    let frame = args.Frame()?;
                    let Ok(frame2) = frame.cast::<ICoreWebView2Frame2>() else {
                        // Runtimes without Frame2 cannot deliver an iframe
                        // message with frame proof, so leave it unsubscribed.
                        return Ok(());
                    };
                    let message_tag = frame_message_tag.clone();
                    let handler = FrameWebMessageReceivedEventHandler::create(Box::new(
                        move |_frame, args| {
                            let Some(args) = args else {
                                return Ok(());
                            };
                            let mut message = PWSTR::null();
                            args.TryGetWebMessageAsString(&mut message)?;
                            let payload = CoTaskMemPWSTR::from(message).to_string();
                            let mut source = PWSTR::null();
                            args.Source(&mut source)?;
                            let source = CoTaskMemPWSTR::from(source).to_string();
                            if let Some(webview) =
                                current_native_callback_webview(&message_tag, native_view_id)
                            {
                                webview.enqueue_web_message(
                                    payload,
                                    windows_web_message_frame(false),
                                    WebMessageTransport::WindowsWebMessage,
                                    windows_message_source(source),
                                );
                            }
                            Ok(())
                        },
                    ));
                    let mut frame_token = 0;
                    frame2.add_WebMessageReceived(&handler, &mut frame_token)?;
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| WebViewError::WebView(format!("add_FrameCreated failed: {err}")))?;
    }

    let message_tag = webtag.clone();
    unsafe {
        let mut token = 0;
        webview
            .add_WebMessageReceived(
                &WebMessageReceivedEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };

                    let mut message = PWSTR::null();
                    args.TryGetWebMessageAsString(&mut message)?;
                    let payload = CoTaskMemPWSTR::from(message).to_string();
                    let mut source = PWSTR::null();
                    args.Source(&mut source)?;
                    let source = CoTaskMemPWSTR::from(source).to_string();

                    if let Some(webview) =
                        current_native_callback_webview(&message_tag, native_view_id)
                    {
                        // The CoreWebView2 event is emitted only by the main
                        // document. Snapshot the normalizer's committed
                        // generation at this callback linearization point.
                        webview.enqueue_web_message(
                            payload,
                            windows_web_message_frame(true),
                            WebMessageTransport::WindowsWebMessage,
                            windows_message_source(source),
                        );
                    } else {
                        log::debug!(
                            "Dropping script message from stale Windows WebView ({})",
                            message_tag
                        );
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| {
                WebViewError::WebView(format!("add_WebMessageReceived failed: {err}"))
            })?;
    }

    let failed_tag = webtag.clone();
    let failed_document_authority = Arc::clone(&document_authority);
    unsafe {
        let mut token = 0;
        webview
            .add_ProcessFailed(
                &ProcessFailedEventHandler::create(Box::new(move |_sender, _args| {
                    if let Some(intent) = failed_document_authority.revoke_pending() {
                        normalizer::revoke_trusted_load(&failed_tag, native_view_id, intent);
                    }
                    normalizer::submit(
                        &failed_tag,
                        native_view_id,
                        NativeSignal::DocumentInvalidated,
                    );
                    if let Some(delegate) =
                        current_native_callback_webview(&failed_tag, native_view_id)
                            .and_then(|webview| webview.get_delegate())
                    {
                        delegate.on_web_content_process_terminated(native_view_id);
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| WebViewError::WebView(format!("add_ProcessFailed failed: {err}")))?;
    }

    for scheme in registered_request_schemes(registered_schemes) {
        let filter = format!("{scheme}://*");
        let filter = CoTaskMemPWSTR::from(filter.as_str());
        unsafe {
            webview
                .AddWebResourceRequestedFilter(
                    *filter.as_ref().as_pcwstr(),
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                )
                .map_err(|err| {
                    WebViewError::WebView(format!(
                        "AddWebResourceRequestedFilter failed for {scheme}: {err}"
                    ))
                })?;
        }
    }

    let request_tag = webtag;
    let request_native_view_id = native_view_id;
    let env = env.clone();
    let memory_pages = memory_pages.clone();
    let custom_schemes = webview2_custom_schemes(registered_schemes);
    unsafe {
        let mut token = 0;
        webview
            .add_WebResourceRequested(
                &WebResourceRequestedEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };

                    let request = args.Request()?;
                    let mut resource_context = COREWEBVIEW2_WEB_RESOURCE_CONTEXT::default();
                    args.ResourceContext(&mut resource_context)?;
                    let frame = windows_scheme_request_frame(resource_context);
                    let uri = take_request_string(|slot| request.Uri(slot))?;
                    let method = take_request_string(|slot| request.Method(slot))?;
                    if let Some(html) = find_memory_page(&memory_pages, &uri) {
                        let native_response = build_memory_html_response(&env, html)?;
                        args.SetResponse(&native_response)?;
                        return Ok(());
                    }

                    let body = request
                        .Content()
                        .ok()
                        .and_then(|stream| read_stream_to_end(&stream).ok())
                        .unwrap_or_default();

                    let mut http_request = Request::builder()
                        .method(method.as_str())
                        .uri(uri.as_str())
                        .body(body)
                        .map_err(http_error_to_win)?;
                    populate_request_headers(&request, http_request.headers_mut())?;

                    let scheme = request_scheme(&uri);
                    let response =
                        current_native_callback_webview(&request_tag, request_native_view_id)
                            .and_then(|webview| {
                                webview.handle_contextual_scheme_request(
                                    scheme,
                                    ContextualSchemeRequest::new(
                                        http_request,
                                        webview.native_view_id(),
                                        frame,
                                    ),
                                )
                            });

                    let Some(response) = response else {
                        // PassThrough (or no webview found): leave the response
                        // unset so WebView2 default handling proceeds for real
                        // http/https requests. Only custom/app schemes, which
                        // the network stack cannot resolve, get a synthetic 404.
                        if custom_schemes.iter().any(|custom| custom == scheme) {
                            let native_response =
                                build_webview2_response(&env, not_found_response())?;
                            args.SetResponse(&native_response)?;
                        }
                        return Ok(());
                    };

                    let native_response = build_webview2_response(&env, response)?;
                    args.SetResponse(&native_response)?;
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| {
                WebViewError::WebView(format!("add_WebResourceRequested failed: {err}"))
            })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_callback_identity_rejects_a_reused_tag() {
        let retired = NativeWebViewId::new(1);
        let replacement = NativeWebViewId::new(2);

        assert!(!native_callback_identity_matches(retired, replacement));
        assert!(native_callback_identity_matches(replacement, replacement));
    }

    #[test]
    fn windows_message_source_is_retained_for_diagnostics_only() {
        let source = windows_message_source("https://example.test/frame".to_string());
        assert_eq!(source.reported_url(), Some("https://example.test/frame"));
        assert_eq!(source.reported_origin(), None);
    }

    #[test]
    fn webview2_main_and_frame_message_events_remain_distinct() {
        assert_eq!(windows_web_message_frame(true), WebMessageFrame::TopLevel);
        assert_eq!(windows_web_message_frame(false), WebMessageFrame::Subframe);
    }
}

pub(crate) fn download_request_from_operation(
    operation: &ICoreWebView2DownloadOperation,
) -> WinResult<DownloadRequest> {
    let url = take_request_string(|slot| unsafe { operation.Uri(slot) })?;
    let content_disposition = non_empty(take_request_string(|slot| unsafe {
        operation.ContentDisposition(slot)
    })?);
    let mime_type = non_empty(take_request_string(|slot| unsafe {
        operation.MimeType(slot)
    })?);
    let result_file_path = non_empty(take_request_string(|slot| unsafe {
        operation.ResultFilePath(slot)
    })?);
    let content_length = unsafe {
        let mut total = 0i64;
        operation.TotalBytesToReceive(&mut total)?;
        u64::try_from(total).ok().filter(|value| *value > 0)
    };
    let suggested_filename = result_file_path
        .as_ref()
        .and_then(|path| std::path::Path::new(path).file_name())
        .map(|name| name.to_string_lossy().to_string());

    Ok(DownloadRequest {
        url,
        user_agent: None,
        content_disposition,
        mime_type,
        content_length,
        suggested_filename,
        source_page_url: None,
        cookie: None,
    })
}

pub(crate) fn non_empty(value: String) -> Option<String> {
    let value = value.trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

pub(crate) fn take_request_string(
    getter: impl FnOnce(*mut PWSTR) -> WinResult<()>,
) -> WinResult<String> {
    let mut value = PWSTR::null();
    getter(&mut value)?;
    Ok(CoTaskMemPWSTR::from(value).to_string())
}
