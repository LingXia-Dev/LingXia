//! Presence values: the paint a node enters from (`starting:`) or leaves toward (`exit:`),
//! applied over its resolved paint.
use super::transition::{Resolved, ShadowLayer};
use crate::protocol::field;

/// `base` with the paint fields of a state-style delta (`[field, value, …]`) applied.
pub(crate) fn apply(
    base: &Resolved,
    delta: &str,
    palette: &crate::theme::Palette,
    size: [f32; 2],
) -> Result<Resolved, String> {
    let invalid = || "UI_TRANSITION: invalid presence style".to_string();
    let pairs: Vec<serde_json::Value> = serde_json::from_str(delta).map_err(|_| invalid())?;
    if !pairs.len().is_multiple_of(2) {
        return Err(invalid());
    }
    let mut resolved = base.clone();
    let color = |value: &serde_json::Value| {
        value
            .as_str()
            .ok_or_else(invalid)
            .and_then(|text| palette.resolve(text))
    };
    for pair in pairs.chunks_exact(2) {
        let id = pair[0].as_u64().ok_or_else(invalid)? as usize;
        let value = &pair[1];
        match id {
            field::OPACITY => {
                resolved.opacity = value
                    .as_f64()
                    .filter(|v| (0.0..=1.0).contains(v))
                    .ok_or_else(invalid)?
            }
            field::TRANSFORM => {
                // A subtree that cannot take a paint affine keeps jumping.
                if resolved.transform.is_some() {
                    let source = value.as_str().ok_or_else(invalid)?;
                    let decomposed =
                        crate::scene::transform::StaticTransform::parse(source)?.decomposed(size);
                    resolved.transform = Some(if resolved.translate_only {
                        super::transition::Decomposed {
                            translate: decomposed.translate,
                            ..Default::default()
                        }
                    } else {
                        decomposed
                    });
                }
            }
            field::BACKGROUND => {
                if resolved.background.is_some() {
                    resolved.background = Some(color(value)?);
                }
            }
            field::COLOR => resolved.foreground = color(value)?,
            field::BORDER_COLOR => {
                if resolved.border.is_some() {
                    resolved.border = Some(color(value)?);
                }
            }
            field::BOX_SHADOW if resolved.shadows.is_some() => {
                let source = value.as_str().ok_or_else(invalid)?;
                let foreground = resolved.foreground;
                resolved.shadows = Some(
                    crate::scene::shadow::BoxShadow::parse(source)?
                        .into_iter()
                        .map(|layer| {
                            Ok(ShadowLayer {
                                offset: [f64::from(layer.offset_x), f64::from(layer.offset_y)],
                                blur: f64::from(layer.blur),
                                spread: f64::from(layer.spread),
                                color: if layer.color == "currentColor" {
                                    foreground
                                } else {
                                    palette.resolve(&layer.color)?
                                },
                            })
                        })
                        .collect::<Result<_, String>>()?,
                );
            }
            // Other paint fields do not transition; layout fields are refused by the compiler.
            _ => {}
        }
    }
    Ok(resolved)
}
