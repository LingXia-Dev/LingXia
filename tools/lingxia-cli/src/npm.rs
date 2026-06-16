pub(crate) fn command() -> &'static str {
    if cfg!(windows) { "npm.cmd" } else { "npm" }
}

#[cfg(test)]
mod tests {
    #[test]
    fn resolves_platform_npm_command() {
        if cfg!(windows) {
            assert_eq!(super::command(), "npm.cmd");
        } else {
            assert_eq!(super::command(), "npm");
        }
    }
}
