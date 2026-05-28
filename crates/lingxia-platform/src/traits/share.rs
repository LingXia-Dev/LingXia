use crate::error::PlatformError;

#[derive(Debug, Clone)]
pub struct ShareRequest {
    pub title: Option<String>,
    pub text: Option<String>,
    pub url: Option<String>,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ShareResult {
    pub completed: Option<bool>,
}

pub trait ShareService: Send + Sync + 'static {
    fn share(
        &self,
        request: ShareRequest,
    ) -> impl std::future::Future<Output = Result<ShareResult, PlatformError>> + Send;
}
