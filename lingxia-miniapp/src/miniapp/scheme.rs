use crate::MiniAppRuntime;
use http::{Request, Response, StatusCode};

pub fn lingxia_handler(
    platform: &(dyn MiniAppRuntime + Send + Sync),
    req: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let uri = req.uri();

    // Get the path part after lingxia://
    let path = uri.path().trim_start_matches('/');

    // Handle home miniapp
    let asset_path = if path.starts_with("home/") {
        path.to_string()
    } else {
        format!("home/{}", path)
    };

    // Read the asset using the platform implementation
    match platform.read_asset(&asset_path) {
        Ok(data) => {
            // Determine MIME type based on file extension
            let mime_type = if asset_path.ends_with(".html") {
                "text/html"
            } else if asset_path.ends_with(".js") {
                "application/javascript"
            } else if asset_path.ends_with(".css") {
                "text/css"
            } else {
                "application/octet-stream"
            };

            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", mime_type)
                .header("Content-Length", data.len().to_string())
                .body(data)
                .unwrap()
        }
        Err(_) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("Content-Type", "text/html")
            .body(
                platform
                    .read_asset("404.html")
                    .unwrap_or("Not Found".into()),
            )
            .unwrap(),
    }
}
