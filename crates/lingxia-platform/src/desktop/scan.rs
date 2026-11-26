use crate::traits::media_interaction::{ScanCodeRequest, ScanType};
use lingxia_error::Error;
use rxing::BarcodeFormat;

/// Desktop implementation of scan_code: opens a file dialog to pick an image, then scans it.
pub async fn scan_code_desktop(request: ScanCodeRequest) -> Result<String, Error> {
    let scan_types = request.scan_types.clone();
    let result = crate::rt::blocking(move || pick_and_scan(&scan_types)).await?;
    let (scan_result, scan_type) = result.ok_or_else(|| Error::business(2000, "scan canceled"))?;
    Ok(format!(
        r#"{{"scanResult":"{}","scanType":"{}"}}"#,
        escape_json(&scan_result),
        escape_json(&scan_type)
    ))
}

fn pick_and_scan(scan_types: &[ScanType]) -> Result<Option<(String, String)>, Error> {
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
        .map_err(|e| Error::internal(format!("Failed to scan image: {}", e)))?;

    for result in &results {
        let format = result.getBarcodeFormat();
        if hints.is_empty() || hints.contains(format) {
            return Ok(Some((
                result.getText().to_string(),
                format_to_type_string(format),
            )));
        }
    }

    if hints.is_empty() {
        return Err(Error::internal(
            "No barcode or QR code found in image".to_string(),
        ));
    }

    Err(Error::internal(
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
        BarcodeFormat::QR_CODE => "qr_code",
        BarcodeFormat::EAN_8 => "ean_8",
        BarcodeFormat::EAN_13 => "ean_13",
        BarcodeFormat::CODE_39 => "code_39",
        BarcodeFormat::CODE_93 => "code_93",
        BarcodeFormat::CODE_128 => "code_128",
        BarcodeFormat::ITF => "itf",
        BarcodeFormat::UPC_A => "upc_a",
        BarcodeFormat::UPC_E => "upc_e",
        BarcodeFormat::DATA_MATRIX => "datamatrix",
        BarcodeFormat::PDF_417 => "pdf417",
        _ => "unknown",
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
