use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{ClassType, msg_send};
use objc2_foundation::{NSBundle, NSData, NSFileManager, NSString, NSURL};
use std::path::Path;
use std::sync::OnceLock;

/// Cached bundles for resource lookup (app bundle, SDK bundle)
/// Initialized once on first access to avoid repeated bundle detection
fn get_resource_bundles() -> &'static [Retained<NSBundle>] {
    static BUNDLES: OnceLock<Vec<Retained<NSBundle>>> = OnceLock::new();
    BUNDLES.get_or_init(|| {
        let mut bundles = Vec::with_capacity(2);
        unsafe {
            let main_bundle = NSBundle::mainBundle();
            let bundle_type = NSString::from_str("bundle");

            // 1. App bundle (for app.json, lxapp content) - based on bundle identifier
            if let Some(app_bundle) = detect_app_bundle(&main_bundle, &bundle_type) {
                bundles.push(app_bundle);
            }

            // 2. SDK bundle (for webview-bridge.js, 404.html, icons)
            for bundle_name in ["lingxia_lingxia", "LingXia_LingXia"] {
                let bundle_name_ns = NSString::from_str(bundle_name);
                let bundle_path: Option<Retained<NSString>> =
                    msg_send![&main_bundle, pathForResource: &*bundle_name_ns, ofType: &*bundle_type];

                if let Some(path) = bundle_path {
                    let resource_bundle: Option<Retained<NSBundle>> =
                        msg_send![NSBundle::class(), bundleWithPath: &*path];
                    if let Some(bundle) = resource_bundle {
                        bundles.push(bundle);
                        break;
                    }
                }
            }

            // 3. Fallback to main bundle if no SPM bundles found
            if bundles.is_empty() {
                bundles.push(main_bundle);
            }
        }
        bundles
    })
}

/// Detect app bundle based on bundle identifier
fn detect_app_bundle(main_bundle: &NSBundle, bundle_type: &NSString) -> Option<Retained<NSBundle>> {
    unsafe {
        let bundle_identifier: Option<Retained<NSString>> =
            msg_send![main_bundle, bundleIdentifier];
        if let Some(identifier) = bundle_identifier {
            let identifier_str = identifier.to_string();
            if let Some(last_component) = identifier_str.split('.').next_back() {
                let spm_bundle_name = format!("{}_{}", last_component, last_component);
                let bundle_name_ns = NSString::from_str(&spm_bundle_name);
                let bundle_path: Option<Retained<NSString>> = msg_send![main_bundle, pathForResource: &*bundle_name_ns, ofType: &*bundle_type];

                if let Some(path) = bundle_path {
                    let resource_bundle: Option<Retained<NSBundle>> =
                        msg_send![NSBundle::class(), bundleWithPath: &*path];
                    return resource_bundle;
                }
            }
        }

        // Fallback: CFBundleName-based SPM naming (keeps older behavior working)
        let cf_bundle_name_key = NSString::from_str("CFBundleName");
        let bundle_name: Option<Retained<NSString>> =
            msg_send![main_bundle, objectForInfoDictionaryKey: &*cf_bundle_name_key];
        if let Some(name) = bundle_name {
            let name_str = name.to_string();
            let spm_bundle_name = format!("{}_{}", name_str, name_str);
            let bundle_name_ns = NSString::from_str(&spm_bundle_name);
            let bundle_path: Option<Retained<NSString>> =
                msg_send![main_bundle, pathForResource: &*bundle_name_ns, ofType: &*bundle_type];

            if let Some(path) = bundle_path {
                let resource_bundle: Option<Retained<NSBundle>> =
                    msg_send![NSBundle::class(), bundleWithPath: &*path];
                return resource_bundle;
            }
        }

        None
    }
}

/// Read asset data from the bundle resources
/// Returns the asset data as bytes, or empty Vec if not found
pub fn read_asset_data(path: &str) -> Vec<u8> {
    unsafe {
        // Clean the path - remove leading slash if present
        let clean_path = path.strip_prefix('/').unwrap_or(path);
        if clean_path.is_empty() {
            return Vec::new();
        }

        let fallback_path = format!("Resources/{}", clean_path);

        // Try cached bundles (app bundle first, then SDK bundle)
        for bundle in get_resource_bundles() {
            // Try the path as-is first, then fallback to Resources/ subdirectory
            for try_path in [clean_path, fallback_path.as_str()] {
                let (subdirectory, filename) = match try_path.rsplit_once('/') {
                    Some((subdir, file)) if !subdir.is_empty() => (Some(subdir), file),
                    _ => (None, try_path),
                };

                let path_extension = Path::new(filename)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("");
                let name_without_extension = Path::new(filename)
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or(filename);

                // Create NSString objects
                let name_ns = NSString::from_str(name_without_extension);
                let extension_ns = if path_extension.is_empty() {
                    None
                } else {
                    Some(NSString::from_str(path_extension))
                };
                let subdirectory_ns = subdirectory.map(NSString::from_str);

                // Try to find the resource URL
                let resource_url: Option<Retained<NSURL>> = if let Some(subdir_ns) =
                    &subdirectory_ns
                {
                    if let Some(ext_ns) = &extension_ns {
                        msg_send![bundle, URLForResource: &*name_ns, withExtension: &**ext_ns, subdirectory: &**subdir_ns]
                    } else {
                        msg_send![bundle, URLForResource: &*name_ns, withExtension: std::ptr::null::<NSString>(), subdirectory: &**subdir_ns]
                    }
                } else if let Some(ext_ns) = &extension_ns {
                    msg_send![bundle, URLForResource: &*name_ns, withExtension: &**ext_ns]
                } else {
                    msg_send![bundle, URLForResource: &*name_ns, withExtension: std::ptr::null::<NSString>()]
                };

                if let Some(url) = resource_url {
                    // Try to read the data
                    let data: Option<Retained<NSData>> =
                        msg_send![NSData::class(), dataWithContentsOfURL: &*url];

                    if let Some(ns_data) = data {
                        let length: usize = msg_send![&ns_data, length];
                        if length > 0 {
                            let bytes_ptr: *const u8 = msg_send![&ns_data, bytes];
                            let slice = std::slice::from_raw_parts(bytes_ptr, length);
                            return slice.to_vec();
                        }
                    }
                }
            }
        }

        Vec::new()
    }
}

/// List contents of an asset directory
/// Returns array of file/directory names in the directory
pub fn list_asset_directory(dir_path: &str) -> Vec<String> {
    unsafe {
        for bundle in get_resource_bundles() {
            let bundle_resource_path: Option<Retained<NSString>> = msg_send![bundle, resourcePath];

            if let Some(resource_path) = bundle_resource_path {
                let clean_path = dir_path.strip_prefix('/').unwrap_or(dir_path);

                // Build full path
                let full_path = if clean_path.is_empty() {
                    resource_path.to_string()
                } else {
                    format!("{}/{}", resource_path, clean_path)
                };

                let full_path_ns = NSString::from_str(&full_path);

                // Get file manager
                let file_manager = NSFileManager::defaultManager();

                // Try to get directory contents
                let contents: Option<Retained<objc2_foundation::NSArray<NSString>>> = msg_send![
                    &file_manager,
                    contentsOfDirectoryAtPath: &*full_path_ns,
                    error: std::ptr::null_mut::<*mut NSObject>()
                ];

                if let Some(contents_array) = contents {
                    let count: usize = msg_send![&contents_array, count];
                    let mut result = Vec::with_capacity(count);

                    for i in 0..count {
                        let item: Retained<NSString> = msg_send![&contents_array, objectAtIndex: i];
                        let item_str = item.to_string();

                        // Filter out hidden files (starting with .)
                        if !item_str.starts_with('.') {
                            result.push(item_str);
                        }
                    }

                    return result;
                }
            }
        }

        Vec::new()
    }
}
