use crate::i18n::js_internal_error;
use crate::ui::present_action_sheet;
use lingxia_platform::traits::media_interaction::MediaSource;
use lxapp::LxApp;
use rong::JSResult;
use std::sync::Arc;

pub(super) async fn present_source_picker(
    lxapp: &Arc<LxApp>,
    sources: &[MediaSource],
) -> JSResult<Option<MediaSource>> {
    let item_list: Vec<String> = sources
        .iter()
        .map(|source| label_for_media_source(*source).to_string())
        .collect();

    let selection = present_action_sheet(lxapp, item_list, None, None).await?;

    match selection {
        Some(idx) => sources
            .get(idx)
            .copied()
            .ok_or_else(|| js_internal_error("chooseMedia source picker returned invalid index"))
            .map(Some),
        None => Ok(None),
    }
}

fn label_for_media_source(source: MediaSource) -> &'static str {
    match source {
        MediaSource::Album => "Album",
        MediaSource::Camera => "Camera",
    }
}
