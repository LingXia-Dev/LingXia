use super::Platform;
use crate::traits::keyboard::AppKeyboard;

// Windows keyboard injection is not yet implemented; falls back to the
// trait's NotSupported default. macOS is the current devtools target.
impl AppKeyboard for Platform {}
