pub(crate) fn encode_string_callback(
    name: &str,
    args: &[&str],
) -> Result<String, serde_json::Error> {
    let mut payload = Vec::with_capacity(args.len() + 1);
    payload.push(name);
    payload.extend_from_slice(args);
    serde_json::to_string(&payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_payload_preserves_arbitrary_argument_boundaries() {
        let encoded = encode_string_callback(
            "showModal",
            &["Delete a|b?", "100%7C", "quote: \" and newline:\n", "42"],
        )
        .expect("callback payload should serialize");
        let decoded: Vec<String> =
            serde_json::from_str(&encoded).expect("callback payload should deserialize");

        assert_eq!(
            decoded,
            [
                "showModal",
                "Delete a|b?",
                "100%7C",
                "quote: \" and newline:\n",
                "42",
            ]
        );
    }

    #[test]
    fn callback_payload_without_arguments_has_only_the_function_name() {
        let encoded =
            encode_string_callback("exitApp", &[]).expect("callback payload should serialize");
        assert_eq!(encoded, r#"["exitApp"]"#);
    }
}
