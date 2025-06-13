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

        // Look for the SPM resource bundle first
        let bundle_name = NSString::from_str("miniapp_miniapp");
        let bundle_type = NSString::from_str("bundle");

        let bundle_path: Option<Retained<NSString>> =
            msg_send![&main_bundle, pathForResource: &*bundle_name, ofType: &*bundle_type];

        if let Some(path) = bundle_path {
            let resource_bundle: Option<Retained<NSBundle>> =
                msg_send![NSBundle::class(), bundleWithPath: &*path];
            if let Some(bundle) = resource_bundle {
                log::debug!("Using SPM resource bundle: miniapp_miniapp.bundle");
                return bundle;
            }
        }

        // Fallback to main bundle
        log::debug!("Using main bundle for resources");
        main_bundle
    }
}

/// Read asset data from the bundle resources
/// Returns the asset data as bytes, or empty Vec if not found
pub fn read_asset_data(path: &str) -> Vec<u8> {
    unsafe {
        // Get the resource bundle (SPM bundle or main bundle)
        let bundle = get_resource_bundle();

        // Clean the path - remove leading slash if present
        let clean_path = if path.starts_with('/') {
            &path[1..]
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
        } else {
            if let Some(ext_ns) = extension_ns {
                msg_send![&bundle, URLForResource: &*name_ns, withExtension: &*ext_ns]
            } else {
                msg_send![&bundle, URLForResource: &*name_ns, withExtension: std::ptr::null::<NSString>()]
            }
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
            let clean_path = if dir_path.starts_with('/') {
                &dir_path[1..]
            } else {
                dir_path
            };

            // Build full path
            let full_path = if clean_path.is_empty() {
                resource_path.to_string()
            } else {
                format!("{}/{}", resource_path.to_string(), clean_path)
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

