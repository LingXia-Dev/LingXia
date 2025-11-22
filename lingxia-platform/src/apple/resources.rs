use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{ClassType, msg_send};
use objc2_foundation::{NSBundle, NSData, NSFileManager, NSString, NSURL};
use std::path::Path;

/// Get the resource bundle for the package
/// Returns the bundle containing resources, or falls back to main bundle
fn get_resource_bundle() -> Retained<NSBundle> {
    unsafe {
        let main_bundle = NSBundle::mainBundle();

        // First try to auto-detect SPM bundle name using native APIs
        if let Some(detected_bundle_name) = detect_spm_bundle_name() {
            let bundle_name_ns = NSString::from_str(&detected_bundle_name);
            let bundle_type = NSString::from_str("bundle");

            let bundle_path: Option<Retained<NSString>> =
                msg_send![&main_bundle, pathForResource: &*bundle_name_ns, ofType: &*bundle_type];

            if let Some(path) = bundle_path {
                let resource_bundle: Option<Retained<NSBundle>> =
                    msg_send![NSBundle::class(), bundleWithPath: &*path];
                if let Some(bundle) = resource_bundle {
                    log::debug!(
                        "Using auto-detected SPM resource bundle: {}.bundle",
                        detected_bundle_name
                    );
                    return bundle;
                }
            }
        }

        main_bundle
    }
}

/// Auto-detect SPM bundle name using native Bundle APIs
fn detect_spm_bundle_name() -> Option<String> {
    unsafe {
        let main_bundle = NSBundle::mainBundle();

        // Method 1: Based on bundle identifier
        let bundle_identifier: Option<Retained<NSString>> =
            msg_send![&main_bundle, bundleIdentifier];
        if let Some(identifier) = bundle_identifier {
            let identifier_str = identifier.to_string();
            if let Some(last_component) = identifier_str.split('.').next_back() {
                let spm_bundle_name = format!("{}_{}", last_component, last_component);

                // Verify this bundle exists
                let bundle_name_ns = NSString::from_str(&spm_bundle_name);
                let bundle_type = NSString::from_str("bundle");
                let bundle_path: Option<Retained<NSString>> = msg_send![&main_bundle, pathForResource: &*bundle_name_ns, ofType: &*bundle_type];

                if bundle_path.is_some() {
                    log::debug!(
                        "Auto-detected bundle name from identifier: {}",
                        spm_bundle_name
                    );
                    return Some(spm_bundle_name);
                }
            }
        }

        // Method 2: Based on CFBundleName
        let cf_bundle_name_key = NSString::from_str("CFBundleName");
        let bundle_name: Option<Retained<NSString>> =
            msg_send![&main_bundle, objectForInfoDictionaryKey: &*cf_bundle_name_key];
        if let Some(name) = bundle_name {
            let name_str = name.to_string();
            let spm_bundle_name = format!("{}_{}", name_str, name_str);

            let bundle_name_ns = NSString::from_str(&spm_bundle_name);
            let bundle_type = NSString::from_str("bundle");
            let bundle_path: Option<Retained<NSString>> =
                msg_send![&main_bundle, pathForResource: &*bundle_name_ns, ofType: &*bundle_type];

            if bundle_path.is_some() {
                log::debug!(
                    "Auto-detected bundle name from CFBundleName: {}",
                    spm_bundle_name
                );
                return Some(spm_bundle_name);
            }
        }

        None
    }
}

/// Read asset data from the bundle resources
/// Returns the asset data as bytes, or empty Vec if not found
pub fn read_asset_data(path: &str) -> Vec<u8> {
    unsafe {
        // Get the resource bundle (SPM bundle or main bundle)
        let bundle = get_resource_bundle();

        // Clean the path - remove leading slash if present
        let clean_path = if let Some(stripped) = path.strip_prefix('/') {
            stripped
        } else {
            path
        };

        if clean_path.is_empty() {
            return Vec::new();
        }

        // Split path into components
        let components: Vec<&str> = clean_path.split('/').collect();
        if components.is_empty() {
            return Vec::new();
        }

        let filename = components.last().unwrap();
        let path_extension = Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");
        let name_without_extension = Path::new(filename)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(filename);

        // Build subdirectory path if exists
        let subdirectory = if components.len() > 1 {
            Some(components[..components.len() - 1].join("/"))
        } else {
            None
        };

        // Create NSString objects
        let name_ns = NSString::from_str(name_without_extension);
        let extension_ns = if path_extension.is_empty() {
            None
        } else {
            Some(NSString::from_str(path_extension))
        };
        let subdirectory_ns = subdirectory.as_ref().map(|s| NSString::from_str(s));

        // Try to find the resource URL
        let resource_url: Option<Retained<NSURL>> = if let Some(subdir_ns) = subdirectory_ns {
            if let Some(ext_ns) = extension_ns {
                msg_send![&bundle, URLForResource: &*name_ns, withExtension: &*ext_ns, subdirectory: &*subdir_ns]
            } else {
                msg_send![&bundle, URLForResource: &*name_ns, withExtension: std::ptr::null::<NSString>(), subdirectory: &*subdir_ns]
            }
        } else if let Some(ext_ns) = extension_ns {
            msg_send![&bundle, URLForResource: &*name_ns, withExtension: &*ext_ns]
        } else {
            msg_send![&bundle, URLForResource: &*name_ns, withExtension: std::ptr::null::<NSString>()]
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

        Vec::new()
    }
}

/// List contents of an asset directory
/// Returns array of file/directory names in the directory
pub fn list_asset_directory(dir_path: &str) -> Vec<String> {
    unsafe {
        // Get the resource bundle (SPM bundle or main bundle)
        let bundle = get_resource_bundle();

        // Get bundle resource path
        let bundle_resource_path: Option<Retained<NSString>> = msg_send![&bundle, resourcePath];

        if let Some(resource_path) = bundle_resource_path {
            // Clean the directory path
            let clean_path = if let Some(stripped) = dir_path.strip_prefix('/') {
                stripped
            } else {
                dir_path
            };

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
            let contents: Option<Retained<objc2_foundation::NSArray<NSString>>> = msg_send![&file_manager, contentsOfDirectoryAtPath: &*full_path_ns, error: std::ptr::null_mut::<*mut NSObject>()];

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

        Vec::new()
    }
}
