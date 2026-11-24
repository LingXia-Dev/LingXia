//! WebView2 document-restore evidence.

/// Whether a WebView2 source callback is evidence of a restored document that
/// bypassed the normal navigation-start invalidation path.
///
/// `HistoryChanged` cannot make this decision: it also fires for same-document
/// history mutations. `SourceChanged.IsNewDocument` is the defensive evidence,
/// while the shared binding tells us whether `NavigationStarting` already
/// revoked the preceding document.
pub(super) fn source_change_restores_document(
    is_new_document: bool,
    preceding_document_is_still_bound: bool,
) -> bool {
    is_new_document && preceding_document_is_still_bound
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_document_over_a_still_bound_one_is_a_restore() {
        assert!(source_change_restores_document(true, true));
    }

    #[test]
    fn navigation_start_invalidates_before_new_document_source_change() {
        assert!(!source_change_restores_document(true, false));
    }

    #[test]
    fn same_document_history_changes_never_revoke_authority() {
        // pushState/replaceState and fragment navigation may emit source and
        // history callbacks without creating a new document.
        assert!(!source_change_restores_document(false, true));
        assert!(!source_change_restores_document(false, false));
    }
}
