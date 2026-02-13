use crate::error::PlatformError;
use crate::traits::media_interaction::{ScanCodeRequest, ScanType};
use rxing::BarcodeFormat;

/// Desktop implementation of scan_code: opens a file dialog to pick an image, then scans it.
pub fn scan_code_desktop(request: ScanCodeRequest) -> Result<(), PlatformError> {
    let callback_id = request.callback_id;
    let scan_types = request.scan_types.clone();

    std::thread::spawn(move || {
        let result = pick_and_scan(&scan_types);
        match result {
            Ok(Some((scan_result, scan_type))) => {
                let payload = format!(
                    r#"{{"scanResult":"{}","scanType":"{}"}}"#,
                    escape_json(&scan_result),
                    escape_json(&scan_type)
                );
                let _ = lingxia_messaging::invoke_callback(callback_id, Ok(payload));
            }
            Ok(None) => {
                // User cancelled
                let _ = lingxia_messaging::invoke_callback(callback_id, Err(2000));
            }
            Err(e) => {
                log::error!("scan_code_desktop error: {}", e);
                let _ = lingxia_messaging::invoke_callback(callback_id, Err(1002));
            }
        }
    });

    Ok(())
}

fn pick_and_scan(scan_types: &[ScanType]) -> Result<Option<(String, String)>, PlatformError> {
    let file = rfd::FileDialog::new()
        .add_filter(
            "Images",
            &["png", "jpg", "jpeg", "bmp", "gif", "webp", "tiff", "tif"],
        )
        .set_title("Select image to scan")
        .pick_file();

    let path = match file {
        Some(p) => p,
        None => return Ok(None),
    };

    let hints = build_hints(scan_types);
    let results = rxing::helpers::detect_multiple_in_file(path.to_string_lossy().as_ref())
        .map_err(|e| PlatformError::Platform(format!("Failed to scan image: {}", e)))?;

    // Filter by requested scan types if specified
    for result in &results {
        let format = result.getBarcodeFormat();
        if hints.is_empty() || hints.contains(&format) {
            let scan_result = result.getText().to_string();
            let scan_type = format_to_type_string(&format);
            return Ok(Some((scan_result, scan_type)));
        }
    }

    if hints.is_empty() {
        return Err(PlatformError::Platform(
            "No barcode or QR code found in image".to_string(),
        ));
    }

    Err(PlatformError::Platform(
        "No code matching requested scan types found in image".to_string(),
    ))
}

fn build_hints(scan_types: &[ScanType]) -> Vec<BarcodeFormat> {
    let mut formats = Vec::new();
    for t in scan_types {
        match t {
            ScanType::QrCode => formats.push(BarcodeFormat::QR_CODE),
            ScanType::BarCode => {
                formats.push(BarcodeFormat::EAN_8);
                formats.push(BarcodeFormat::EAN_13);
                formats.push(BarcodeFormat::CODE_39);
                formats.push(BarcodeFormat::CODE_93);
                formats.push(BarcodeFormat::CODE_128);
                formats.push(BarcodeFormat::ITF);
                formats.push(BarcodeFormat::UPC_A);
                formats.push(BarcodeFormat::UPC_E);
            }
            ScanType::DataMatrix => formats.push(BarcodeFormat::DATA_MATRIX),
            ScanType::Pdf417 => formats.push(BarcodeFormat::PDF_417),
        }
    }
    formats
}

fn format_to_type_string(format: &BarcodeFormat) -> String {
    match format {
        BarcodeFormat::QR_CODE => "QR_CODE",
        BarcodeFormat::EAN_8 => "EAN_8",
        BarcodeFormat::EAN_13 => "EAN_13",
        BarcodeFormat::CODE_39 => "CODE_39",
        BarcodeFormat::CODE_93 => "CODE_93",
        BarcodeFormat::CODE_128 => "CODE_128",
        BarcodeFormat::ITF => "ITF",
        BarcodeFormat::UPC_A => "UPC_A",
        BarcodeFormat::UPC_E => "UPC_E",
        BarcodeFormat::DATA_MATRIX => "DATA_MATRIX",
        BarcodeFormat::PDF_417 => "PDF_417",
        _ => "UNKNOWN",
    }
    .to_string()
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
