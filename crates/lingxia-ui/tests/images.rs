use lingxia_ui::{Renderer, image::Bitmap, schema::decode};
use serde_json::json;
use std::collections::HashMap;

#[test]
fn image_wire_requires_source_and_rejects_invalid_fields_and_children() {
    let valid = json!([7, -1, "", "photo", "", 2, 19, "public/a.png", 20, "contain"]);
    assert_eq!(decode(valid.as_array().unwrap()).unwrap()[0].tag, 7);
    for invalid in [
        json!([7, -1, "", "", "", 0]),
        json!([7, -1, "", "", "", 1, 19, 42]),
        json!([7, -1, "", "", "", 1, 19, ""]),
        json!([7, -1, "", "", "", 2, 19, "a", 20, "stretch"]),
        json!([0, -1, "", "", "", 1, 19, "a"]),
        json!([7, -1, "", "", "", 1, 19, "a", 2, 0, "", "", "child", 0]),
    ] {
        assert!(decode(invalid.as_array().unwrap()).is_err(), "{invalid}");
    }
}

#[test]
fn visible_image_budget_includes_pending_sources() {
    use image::ImageEncoder;
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&[255, 0, 0, 255], 1, 1, image::ExtendedColorType::Rgba8)
        .unwrap();
    let image = Bitmap::decode(&png).unwrap();
    for count in [128, 129] {
        let mut buffer = json!([0, -1, "", "", "", 0]).as_array().unwrap().clone();
        let mut images = HashMap::new();
        for index in 0..count {
            let source = format!("image-{index}");
            buffer.extend(
                json!([7, 0, source, "", "", 3, 0, 1, 1, 1, 19, source])
                    .as_array()
                    .unwrap()
                    .iter()
                    .cloned(),
            );
            images.insert(source, image.clone());
        }
        let mut display = Renderer::default()
            .layout(&decode(&buffer).unwrap(), 256, 256, 1.0)
            .unwrap();
        assert_eq!(display.image_sources().count(), count);
        if count == 128 {
            display.bind_images(&images).unwrap();
            assert_eq!(display.loaded_image_count(), 128);
        } else {
            assert!(display.bind_images(&HashMap::new()).is_err());
            assert!(display.bind_images(&images).is_err());
        }
    }
}
