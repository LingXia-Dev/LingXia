use http::{Request, Response, StatusCode};

use crate::miniapp::MiniApp;

impl MiniApp {
    pub(crate) fn lingxia_handler(&self, req: Request<Vec<u8>>) -> Response<Vec<u8>> {
        let uri = req.uri();

        // Get the path part after lingxia://
        let path = uri.path().trim_start_matches('/');

        // Try to read the asset from app directory
        let file_result = self.read_bytes(path);

        match file_result {
            Ok(data) => {
                // Determine MIME type based on file extension
                let mime_type = if path.ends_with(".html") {
                    "text/html"
                } else if path.ends_with(".js") {
                    "application/javascript"
                } else if path.ends_with(".css") {
                    "text/css"
                } else if path.ends_with(".png") {
                    "image/png"
                } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
                    "image/jpeg"
                } else if path.ends_with(".svg") {
                    "image/svg+xml"
                } else if path.ends_with(".json") {
                    "application/json"
                } else {
                    "application/octet-stream"
                };

                Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", mime_type)
                    .header("Content-Length", data.len().to_string())
                    .body(data)
                    .unwrap_or_else(|_| {
                        Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Vec::new())
                            .unwrap()
                    })
            }
            Err(_) => {
                // Return a 404 Not Found response
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header("Content-Type", "text/html")
                    .body(match self.controller.read_asset("404.html") {
                        Ok(mut reader) => {
                            let mut data = Vec::new();
                            if reader.read_to_end(&mut data).is_ok() {
                                data
                            } else {
                                "Not Found".as_bytes().to_vec()
                            }
                        }
                        Err(_) => "Not Found".as_bytes().to_vec(),
                    })
                    .unwrap()
            }
        }
    }
}
