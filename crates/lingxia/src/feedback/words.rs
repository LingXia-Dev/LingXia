use lingxia_logic::I18nKey;

/// A dialog's default words, in the calling lxapp's page language so they match its title and
/// message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Words {
    pub ok: &'static str,
    pub confirm: &'static str,
    pub cancel: &'static str,
    /// An action sheet's accessible name.
    pub actions: &'static str,
}

impl Words {
    pub fn new(language: &str) -> Self {
        Self {
            ok: I18nKey::CommonOk.get(language),
            confirm: I18nKey::CommonConfirm.get(language),
            cancel: I18nKey::CommonCancel.get(language),
            actions: I18nKey::CommonActions.get(language),
        }
    }
}
