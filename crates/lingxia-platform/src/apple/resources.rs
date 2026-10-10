use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::runtime::{AnyObject, Sel};
use objc2::{ClassType, msg_send, sel};
use objc2_foundation::{NSBundle, NSData, NSFileManager, NSString, NSURL};
use std::path::Path;
use std::sync::OnceLock;

#[inline]
unsafe fn nsdata_bytes_ptr_unchecked(ns_data: &Retained<NSData>) -> *const u8 {
    // Avoid objc2's debug-time method signature verification for `-[NSData bytes]`.
    //
    // On some Apple OS versions, the runtime type encoding for this method is
    // not `^v` as expected, which can cause a panic in debug builds.
    let obj: *const AnyObject = Retained::as_ptr(ns_data).cast();
    let sel: Sel = sel!(bytes);
    let func: unsafe extern "C" fn(*const AnyObject, Sel) -> *const core::ffi::c_void =
        unsafe { core::mem::transmute(objc2::ffi::objc_msgSend as *const ()) };
    unsafe { func(obj, sel) }.cast()
}

/// Bundles searched for assets, in order: the main bundle, which carries the
/// host's resources (the CLI merges them in at build time), then the SDK bundle.
fn get_resource_bundles() -> &'static [Retained<NSBundle>] {
    static BUNDLES: OnceLock<Vec<Retained<NSBundle>>> = OnceLock::new();
    BUNDLES.get_or_init(|| unsafe {
        let main_bundle = NSBundle::mainBundle();
        let mut bundles = vec![main_bundle.clone()];
        let bundle_type = NSString::from_str("bundle");
        for bundle_name in ["lingxia_lingxia", "LingXia_LingXia"] {
            let bundle_name_ns = NSString::from_str(bundle_name);
            let bundle_path: Option<Retained<NSString>> =
                msg_send![&main_bundle, pathForResource: &*bundle_name_ns, ofType: &*bundle_type];
            let bundle: Option<Retained<NSBundle>> = match bundle_path {
                Some(path) => msg_send![NSBundle::class(), bundleWithPath: &*path],
                None => None,
            };
            if let Some(bundle) = bundle {
                bundles.push(bundle);
                break;
            }
        }
        bundles
    })
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
                            let bytes_ptr: *const u8 = nsdata_bytes_ptr_unchecked(&ns_data);
                            let slice = std::slice::from_raw_parts(bytes_ptr.cast::<u8>(), length);
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
        let clean_path = dir_path.strip_prefix('/').unwrap_or(dir_path);

        for bundle in get_resource_bundles() {
            let bundle_resource_path: Option<Retained<NSString>> = msg_send![bundle, resourcePath];

            if let Some(resource_path) = bundle_resource_path {
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

                        result.push(item_str);
                    }

                    return result;
                }
            }
        }

        Vec::new()
    }
}
