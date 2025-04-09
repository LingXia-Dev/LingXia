use crate::AssetReader;
use http::{Request, Response, StatusCode};

const NOT_FOUND_HTML: &str = include_str!("404.html");

pub fn lingxia_handler(
    asset_reader: &(dyn AssetReader + Send + Sync),
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

    // Get the stored asset manager and read the file
    match asset_reader.read_asset(&asset_path) {
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
            .body(NOT_FOUND_HTML.as_bytes().to_vec())
            .unwrap(),
    }
}

