#[macro_export]
macro_rules! verbose {
    ($appid:expr, $($arg:tt)*) => ({
        if let Ok(miniapp) = $crate::miniapp::get().lock() {
            miniapp.platform.log($crate::miniapp::LogLevel::Verbose, &format!($($arg)*));
        }
    });
}

#[macro_export]
macro_rules! debug {
    ($appid:expr, $($arg:tt)*) => ({
        if let Ok(miniapp) = $crate::miniapp::get().lock() {
            miniapp.platform.log($crate::miniapp::LogLevel::Debug, &format!($($arg)*));
        }
    });
}

#[macro_export]
macro_rules! info {
    ($appid:expr, $($arg:tt)*) => ({
        if let Ok(miniapp) = $crate::miniapp::get().lock() {
            miniapp.platform.log($crate::miniapp::LogLevel::Info, &format!($($arg)*));
        }
    });
}

#[macro_export]
macro_rules! warn {
    ($appid:expr, $($arg:tt)*) => ({
        if let Ok(miniapp) = $crate::miniapp::get().lock() {
            miniapp.platform.log($crate::miniapp::LogLevel::Warn, &format!($($arg)*));
        }
    });
}

#[macro_export]
macro_rules! error {
    ($appid:expr, $($arg:tt)*) => ({
        if let Ok(miniapp) = $crate::miniapp::get().lock() {
            miniapp.platform.log($crate::miniapp::LogLevel::Error, &format!($($arg)*));
        }
    });
}

