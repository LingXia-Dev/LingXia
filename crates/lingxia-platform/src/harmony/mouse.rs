use super::app::Platform;
use crate::traits::mouse::AppMouse;

impl AppMouse for Platform {}

impl crate::traits::gesture::AppGesture for Platform {}
