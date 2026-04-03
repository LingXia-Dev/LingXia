use crate::error::PlatformError;
use crate::traits::media_interaction::{ScanCodeRequest, ScanType};
use rxing::BarcodeFormat;

/// Desktop implementation of scan_code: opens a file dialog to pick an image, then scans it.
pub async fn scan_code_desktop(request: ScanCodeRequest) -> Result<String, PlatformError> {
    let scan_types = request.scan_types.clone();
    let result = crate::rt::blocking(move || pick_and_scan(&scan_types)).await?;
    let (scan_result, scan_type) = result.ok_or(PlatformError::BusinessError(2000))?;
    Ok(format!(
        r#"{{"scanResult":"{}","scanType":"{}"}}"#,
        escape_json(&scan_result),
        escape_json(&scan_type)
    ))
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

    for result in &results {
        let format = result.getBarcodeFormat();
        if hints.is_empty() || hints.contains(&format) {
            return Ok(Some((
                result.getText().to_string(),
                format_to_type_string(&format),
            )));
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
