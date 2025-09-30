use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::{ModalOptions, PickerType, ToastOptions, UserFeedback};
use jni::objects::{JClass, JObject, JValue};
use std::collections::HashMap;

impl UserFeedback for Platform {
    fn show_toast(&self, options: ToastOptions) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = lingxia_webview::get_env()?;

            // Get the LxApp class
            let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)?
                .as_obj()
                .into();

            // Convert parameters to Java objects
            let title_jstring = env.new_string(&options.title)?;
            let title_obj: JObject = title_jstring.into();

            let image_obj: JObject = if let Some(image) = &options.image {
                env.new_string(image)?.into()
            } else {
                JObject::null()
            };

            // Call the static showToast method on LxApp
            env.call_static_method(
                lxapp_class,
                "showToast",
                "(Ljava/lang/String;ILjava/lang/String;DZI)V",
                &[
                    JValue::Object(&title_obj),
                    JValue::Int(options.icon as i32),
                    JValue::Object(&image_obj),
                    JValue::Double(options.duration),
                    JValue::Bool(options.mask as u8),
                    JValue::Int(options.position as i32),
                ],
            )?;
            Ok(())
        }() {
            Ok(_) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to show toast: {}",
                e
            ))),
        }
    }

    fn hide_toast(&self) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = lingxia_webview::get_env()?;

            // Get the LxApp class
            let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)?
                .as_obj()
                .into();

            // Call the static hideToast method on LxApp
            env.call_static_method(lxapp_class, "hideToast", "()V", &[])?;
            Ok(())
        }() {
            Ok(_) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to hide toast: {}",
                e
            ))),
        }
    }

    fn show_modal(&self, options: ModalOptions, callback_id: u64) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = lingxia_webview::get_env()?;

            // Get the LxApp class
            let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)?
                .as_obj()
                .into();

            // Create parameters map
            let mut params = HashMap::new();
            params.insert("title", options.title.as_str());
            params.insert("content", options.content.as_str());
            params.insert("cancelText", options.cancel_text.as_str());
            params.insert("confirmText", options.confirm_text.as_str());

            if let Some(ref cancel_color) = options.cancel_color {
                params.insert("cancelColor", cancel_color.as_str());
            }
            if let Some(ref confirm_color) = options.confirm_color {
                params.insert("confirmColor", confirm_color.as_str());
            }

            // Create HashMap object
            let hashmap_class = env.find_class("java/util/HashMap")?;
            let hashmap = env.new_object(hashmap_class, "()V", &[])?;

            // Put string values
            for (key, value) in params {
                let key_jstring = env.new_string(key)?;
                let value_jstring = env.new_string(value)?;
                env.call_method(
                    &hashmap,
                    "put",
                    "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                    &[
                        JValue::Object(&key_jstring.into()),
                        JValue::Object(&value_jstring.into()),
                    ],
                )?;
            }

            // Put boolean values
            let show_cancel_key = env.new_string("showCancel")?;
            let show_cancel_value = env.call_static_method(
                "java/lang/Boolean",
                "valueOf",
                "(Z)Ljava/lang/Boolean;",
                &[JValue::Bool(options.show_cancel as u8)],
            )?;
            env.call_method(
                &hashmap,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[
                    JValue::Object(&show_cancel_key.into()),
                    show_cancel_value.borrow(),
                ],
            )?;

            // Add callback_id to the parameters
            let callback_id_key = env.new_string("callbackId")?;
            let callback_id_value = env.call_static_method(
                "java/lang/Long",
                "valueOf",
                "(J)Ljava/lang/Long;",
                &[JValue::Long(callback_id as i64)],
            )?;
            env.call_method(
                &hashmap,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[
                    JValue::Object(&callback_id_key.into()),
                    callback_id_value.borrow(),
                ],
            )?;

            // Call the static showModal method on LxApp
            env.call_static_method(
                lxapp_class,
                "showModal",
                "(Ljava/util/Map;)V",
                &[JValue::Object(&hashmap)],
            )?;

            Ok(())
        }() {
            Ok(()) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to show modal: {}",
                e
            ))),
        }
    }

    fn show_action_sheet(
        &self,
        options: Vec<String>,
        cancel_text: String,
        item_color: String,
        callback_id: u64,
    ) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = lingxia_webview::get_env()?;

            // Get the LxApp class
            let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)?
                .as_obj()
                .into();

            // Create HashMap object
            let hashmap_class = env.find_class("java/util/HashMap")?;
            let hashmap = env.new_object(hashmap_class, "()V", &[])?;

            // Convert options to ArrayList
            let arraylist_class = env.find_class("java/util/ArrayList")?;
            let options_list = env.new_object(arraylist_class, "()V", &[])?;

            for option in &options {
                let option_jstring = env.new_string(option)?;
                env.call_method(
                    &options_list,
                    "add",
                    "(Ljava/lang/Object;)Z",
                    &[JValue::Object(&option_jstring.into())],
                )?;
            }

            // Put options in hashmap
            let options_key = env.new_string("itemList")?;
            env.call_method(
                &hashmap,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[
                    JValue::Object(&options_key.into()),
                    JValue::Object(&options_list),
                ],
            )?;

            // Put cancel text
            let cancel_text_key = env.new_string("cancelText")?;
            let cancel_text_value = env.new_string(&cancel_text)?;
            env.call_method(
                &hashmap,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[
                    JValue::Object(&cancel_text_key.into()),
                    JValue::Object(&cancel_text_value.into()),
                ],
            )?;

            // Put item color
            let item_color_key = env.new_string("itemColor")?;
            let item_color_value = env.new_string(&item_color)?;
            env.call_method(
                &hashmap,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[
                    JValue::Object(&item_color_key.into()),
                    JValue::Object(&item_color_value.into()),
                ],
            )?;

            // Add callback_id to the parameters
            let callback_id_key = env.new_string("callbackId")?;
            let callback_id_value = env.call_static_method(
                "java/lang/Long",
                "valueOf",
                "(J)Ljava/lang/Long;",
                &[JValue::Long(callback_id as i64)],
            )?;
            env.call_method(
                &hashmap,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[
                    JValue::Object(&callback_id_key.into()),
                    callback_id_value.borrow(),
                ],
            )?;

            // Call the static showActionSheet method on LxApp
            env.call_static_method(
                lxapp_class,
                "showActionSheet",
                "(Ljava/util/Map;)V",
                &[JValue::Object(&hashmap)],
            )?;

            Ok(())
        }() {
            Ok(()) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to show action sheet: {}",
                e
            ))),
        }
    }

    fn show_picker(
        &self,
        picker_type: PickerType,
        cancel_text: String,
        cancel_button_color: String,
        cancel_text_color: String,
        confirm_text: String,
        confirm_button_color: String,
        confirm_text_color: String,
        callback_id: u64,
    ) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = lingxia_webview::get_env()?;

            // Get the LxApp class
            let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)?
                .as_obj()
                .into();

            // Create HashMap object
            let hashmap_class = env.find_class("java/util/HashMap")?;
            let hashmap = env.new_object(hashmap_class, "()V", &[])?;

            // Convert picker type to new API format
            let arraylist_class = env.find_class("java/util/ArrayList")?;

            match picker_type {
                PickerType::SingleColumn { items } => {
                    // Set mode
                    let mode_jstring = env.new_string("selector")?;
                    env.call_method(
                        &hashmap,
                        "put",
                        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                        &[
                            JValue::Object(&env.new_string("mode")?.into()),
                            JValue::Object(&mode_jstring.into()),
                        ],
                    )?;

                    // Set range (List<String> for single column)
                    let range_list = env.new_object(&arraylist_class, "()V", &[])?;
                    for item in items {
                        let item_jstring = env.new_string(&item)?;
                        env.call_method(
                            &range_list,
                            "add",
                            "(Ljava/lang/Object;)Z",
                            &[JValue::Object(&item_jstring.into())],
                        )?;
                    }
                    env.call_method(
                        &hashmap,
                        "put",
                        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                        &[
                            JValue::Object(&env.new_string("range")?.into()),
                            JValue::Object(&range_list),
                        ],
                    )?;

                    // Set value (initial selection index)
                    let value_list = env.new_object(&arraylist_class, "()V", &[])?;
                    let zero_integer =
                        env.new_object("java/lang/Integer", "(I)V", &[JValue::Int(0)])?;
                    env.call_method(
                        &value_list,
                        "add",
                        "(Ljava/lang/Object;)Z",
                        &[JValue::Object(&zero_integer)],
                    )?;
                    env.call_method(
                        &hashmap,
                        "put",
                        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                        &[
                            JValue::Object(&env.new_string("value")?.into()),
                            JValue::Object(&value_list),
                        ],
                    )?;
                }
                PickerType::DualColumn {
                    first_column,
                    second_column,
                } => {
                    // Set mode
                    let mode_jstring = env.new_string("multiSelector")?;
                    env.call_method(
                        &hashmap,
                        "put",
                        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                        &[
                            JValue::Object(&env.new_string("mode")?.into()),
                            JValue::Object(&mode_jstring.into()),
                        ],
                    )?;

                    // Set range (List<List<String>> for dual column)
                    let range_list = env.new_object(&arraylist_class, "()V", &[])?;

                    // Add first column
                    let first_column_list = env.new_object(&arraylist_class, "()V", &[])?;
                    for item in first_column.iter() {
                        let item_jstring = env.new_string(&item)?;
                        env.call_method(
                            &first_column_list,
                            "add",
                            "(Ljava/lang/Object;)Z",
                            &[JValue::Object(&item_jstring.into())],
                        )?;
                    }
                    env.call_method(
                        &range_list,
                        "add",
                        "(Ljava/lang/Object;)Z",
                        &[JValue::Object(&first_column_list)],
                    )?;

                    // Add second column
                    let second_column_list = env.new_object(&arraylist_class, "()V", &[])?;
                    for item in second_column.iter() {
                        let item_jstring = env.new_string(&item)?;
                        env.call_method(
                            &second_column_list,
                            "add",
                            "(Ljava/lang/Object;)Z",
                            &[JValue::Object(&item_jstring.into())],
                        )?;
                    }
                    env.call_method(
                        &range_list,
                        "add",
                        "(Ljava/lang/Object;)Z",
                        &[JValue::Object(&second_column_list)],
                    )?;

                    // Set range in hashmap
                    env.call_method(
                        &hashmap,
                        "put",
                        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                        &[
                            JValue::Object(&env.new_string("range")?.into()),
                            JValue::Object(&range_list),
                        ],
                    )?;

                    // Set value (initial selection indices)
                    let value_list = env.new_object(&arraylist_class, "()V", &[])?;
                    let zero_integer1 =
                        env.new_object("java/lang/Integer", "(I)V", &[JValue::Int(0)])?;
                    let zero_integer2 =
                        env.new_object("java/lang/Integer", "(I)V", &[JValue::Int(0)])?;
                    env.call_method(
                        &value_list,
                        "add",
                        "(Ljava/lang/Object;)Z",
                        &[JValue::Object(&zero_integer1)],
                    )?;
                    env.call_method(
                        &value_list,
                        "add",
                        "(Ljava/lang/Object;)Z",
                        &[JValue::Object(&zero_integer2)],
                    )?;
                    env.call_method(
                        &hashmap,
                        "put",
                        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                        &[
                            JValue::Object(&env.new_string("value")?.into()),
                            JValue::Object(&value_list),
                        ],
                    )?;
                }
                PickerType::DualColumnCascading {
                    first_column,
                    cascading_data,
                } => {
                    // Set mode
                    let mode_jstring = env.new_string("multiSelector")?;
                    env.call_method(
                        &hashmap,
                        "put",
                        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                        &[
                            JValue::Object(&env.new_string("mode")?.into()),
                            JValue::Object(&mode_jstring.into()),
                        ],
                    )?;

                    // Set cascading flag
                    let cascading_jstring = env.new_string("true")?;
                    env.call_method(
                        &hashmap,
                        "put",
                        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                        &[
                            JValue::Object(&env.new_string("cascading")?.into()),
                            JValue::Object(&cascading_jstring.into()),
                        ],
                    )?;

                    // Set range (List<List<String>> for dual column)
                    let range_list = env.new_object(&arraylist_class, "()V", &[])?;

                    // Add first column
                    let first_column_list = env.new_object(&arraylist_class, "()V", &[])?;
                    for item in first_column.iter() {
                        let item_jstring = env.new_string(&item)?;
                        env.call_method(
                            &first_column_list,
                            "add",
                            "(Ljava/lang/Object;)Z",
                            &[JValue::Object(&item_jstring.into())],
                        )?;
                    }
                    env.call_method(
                        &range_list,
                        "add",
                        "(Ljava/lang/Object;)Z",
                        &[JValue::Object(&first_column_list)],
                    )?;

                    // Add cascading data as HashMap
                    let hashmap_class = env.find_class("java/util/HashMap")?;
                    let cascading_hashmap = env.new_object(&hashmap_class, "()V", &[])?;

                    for (key, values) in cascading_data.iter() {
                        let key_jstring = env.new_string(key)?;
                        let values_list = env.new_object(&arraylist_class, "()V", &[])?;

                        for value in values.iter() {
                            let value_jstring = env.new_string(value)?;
                            env.call_method(
                                &values_list,
                                "add",
                                "(Ljava/lang/Object;)Z",
                                &[JValue::Object(&value_jstring.into())],
                            )?;
                        }

                        env.call_method(
                            &cascading_hashmap,
                            "put",
                            "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                            &[
                                JValue::Object(&key_jstring.into()),
                                JValue::Object(&values_list),
                            ],
                        )?;
                    }

                    env.call_method(
                        &range_list,
                        "add",
                        "(Ljava/lang/Object;)Z",
                        &[JValue::Object(&cascading_hashmap)],
                    )?;

                    // Set range in hashmap
                    env.call_method(
                        &hashmap,
                        "put",
                        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                        &[
                            JValue::Object(&env.new_string("range")?.into()),
                            JValue::Object(&range_list),
                        ],
                    )?;

                    // Set value (initial selection indices)
                    let value_list = env.new_object(&arraylist_class, "()V", &[])?;
                    let zero_integer1 =
                        env.new_object("java/lang/Integer", "(I)V", &[JValue::Int(0)])?;
                    let zero_integer2 =
                        env.new_object("java/lang/Integer", "(I)V", &[JValue::Int(0)])?;
                    env.call_method(
                        &value_list,
                        "add",
                        "(Ljava/lang/Object;)Z",
                        &[JValue::Object(&zero_integer1)],
                    )?;
                    env.call_method(
                        &value_list,
                        "add",
                        "(Ljava/lang/Object;)Z",
                        &[JValue::Object(&zero_integer2)],
                    )?;
                    env.call_method(
                        &hashmap,
                        "put",
                        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                        &[
                            JValue::Object(&env.new_string("value")?.into()),
                            JValue::Object(&value_list),
                        ],
                    )?;
                }
            }

            // Put cancel text and colors in hashmap
            let cancel_text_key = env.new_string("cancelText")?;
            let cancel_text_jstring = env.new_string(&cancel_text)?;
            env.call_method(
                &hashmap,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[
                    JValue::Object(&cancel_text_key.into()),
                    JValue::Object(&cancel_text_jstring.into()),
                ],
            )?;

            let cancel_button_color_key = env.new_string("cancelButtonColor")?;
            let cancel_button_color_jstring = env.new_string(&cancel_button_color)?;
            env.call_method(
                &hashmap,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[
                    JValue::Object(&cancel_button_color_key.into()),
                    JValue::Object(&cancel_button_color_jstring.into()),
                ],
            )?;

            let cancel_text_color_key = env.new_string("cancelTextColor")?;
            let cancel_text_color_jstring = env.new_string(&cancel_text_color)?;
            env.call_method(
                &hashmap,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[
                    JValue::Object(&cancel_text_color_key.into()),
                    JValue::Object(&cancel_text_color_jstring.into()),
                ],
            )?;

            // Put confirm text and colors in hashmap
            let confirm_text_key = env.new_string("confirmText")?;
            let confirm_text_jstring = env.new_string(&confirm_text)?;
            env.call_method(
                &hashmap,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[
                    JValue::Object(&confirm_text_key.into()),
                    JValue::Object(&confirm_text_jstring.into()),
                ],
            )?;

            let confirm_button_color_key = env.new_string("confirmButtonColor")?;
            let confirm_button_color_jstring = env.new_string(&confirm_button_color)?;
            env.call_method(
                &hashmap,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[
                    JValue::Object(&confirm_button_color_key.into()),
                    JValue::Object(&confirm_button_color_jstring.into()),
                ],
            )?;

            let confirm_text_color_key = env.new_string("confirmTextColor")?;
            let confirm_text_color_jstring = env.new_string(&confirm_text_color)?;
            env.call_method(
                &hashmap,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[
                    JValue::Object(&confirm_text_color_key.into()),
                    JValue::Object(&confirm_text_color_jstring.into()),
                ],
            )?;

            // Add callback_id to the parameters
            let callback_id_key = env.new_string("callbackId")?;
            let callback_id_value = env.call_static_method(
                "java/lang/Long",
                "valueOf",
                "(J)Ljava/lang/Long;",
                &[JValue::Long(callback_id as i64)],
            )?;
            env.call_method(
                &hashmap,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[
                    JValue::Object(&callback_id_key.into()),
                    callback_id_value.borrow(),
                ],
            )?;

            // Call the static showPicker method on LxApp
            env.call_static_method(
                lxapp_class,
                "showPicker",
                "(Ljava/util/Map;)V",
                &[JValue::Object(&hashmap)],
            )?;

            Ok(())
        }() {
            Ok(()) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to show picker: {}",
                e
            ))),
        }
    }
}
