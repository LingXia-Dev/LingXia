//! Shared resolution logic for sizing keywords (`min-content`, `max-content`, `fit-content`,
//! `fit-content(...)`, and `stretch`) on the `width`/`height` style properties
use crate::geometry::{AbsoluteAxis, Line, Rect, Size};
use crate::style::AvailableSpace;
use crate::tree::{LayoutPartialTree, LayoutPartialTreeExt, NodeId, SizingMode};
use crate::util::sys::f32_max;
use crate::{CompactLength, Dimension};

/// How a sizing keyword resolves to a used size
pub(crate) enum SizingKeywordResolution {
    /// The size is the result of measuring the item under the given available space constraint
    Measure(AvailableSpace),
    /// The size resolves to an exact value without measuring the item
    Exact(f32),
}

/// Resolve an item's size style in one axis if it is a sizing keyword (`min-content`,
/// `max-content`, `fit-content`, `fit-content(...)`, or `stretch`).
///
/// - `stretch_size` is the size the item would take if stretched to fill the available space
///   (available space minus margins). Used by `fit-content` and `stretch`.
/// - `percent_resolution_basis` is the size that percentages resolve against in this axis.
///   Used by `fit-content(<percentage>)`.
///
/// Returns `None` if the size style is not a sizing keyword, or if it cannot
/// be resolved in the current context (in which case it behaves as `auto`).
#[inline]
pub(crate) fn resolve_sizing_keyword(
    style: Dimension,
    stretch_size: Option<f32>,
    percent_resolution_basis: Option<f32>,
) -> Option<SizingKeywordResolution> {
    match style.tag() {
        CompactLength::MIN_CONTENT_TAG => Some(SizingKeywordResolution::Measure(AvailableSpace::MinContent)),
        CompactLength::MAX_CONTENT_TAG => Some(SizingKeywordResolution::Measure(AvailableSpace::MaxContent)),
        CompactLength::FIT_CONTENT_PX_TAG => {
            Some(SizingKeywordResolution::Measure(AvailableSpace::Definite(style.value())))
        }
        CompactLength::FIT_CONTENT_PERCENT_TAG => percent_resolution_basis
            .map(|basis| SizingKeywordResolution::Measure(AvailableSpace::Definite(basis * style.value()))),
        CompactLength::FIT_CONTENT_KEYWORD_TAG => {
            stretch_size.map(|size| SizingKeywordResolution::Measure(AvailableSpace::Definite(size)))
        }
        CompactLength::STRETCH_TAG => stretch_size.map(SizingKeywordResolution::Exact),
        _ => None,
    }
}

/// Resolve an item's `min-*` or `max-*` size style in one axis if it is a sizing keyword, to a
/// border-box size. Intrinsic keywords measure the item's content size (ignoring its own size
/// style) under the keyword's constraint in `axis`; `known_dimensions` and `available_space`
/// supply the other axis.
///
/// Returns `None` without measuring if the style is not a sizing keyword, and `None` if it cannot
/// be resolved in the current context (a minimum then behaves as `auto`, a maximum as `none`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_min_max_sizing_keyword(
    tree: &mut impl LayoutPartialTree,
    node: NodeId,
    style: Dimension,
    axis: AbsoluteAxis,
    known_dimensions: Size<Option<f32>>,
    parent_size: Size<Option<f32>>,
    available_space: Size<AvailableSpace>,
    stretch_size: Option<f32>,
    percent_resolution_basis: Option<f32>,
    vertical_margins_are_collapsible: Line<bool>,
) -> Option<f32> {
    if !style.is_sizing_keyword() {
        return None;
    }
    match resolve_sizing_keyword(style, stretch_size, percent_resolution_basis)? {
        SizingKeywordResolution::Exact(size) => Some(size),
        SizingKeywordResolution::Measure(available) => {
            let (known_dimensions, available_space) = match axis {
                AbsoluteAxis::Horizontal => {
                    (Size { width: None, ..known_dimensions }, Size { width: available, ..available_space })
                }
                AbsoluteAxis::Vertical => {
                    (Size { height: None, ..known_dimensions }, Size { height: available, ..available_space })
                }
            };
            Some(tree.measure_child_size(
                node,
                known_dimensions,
                parent_size,
                available_space,
                SizingMode::ContentSize,
                axis,
                vertical_margins_are_collapsible,
            ))
        }
    }
}

/// Resolve the sizing keywords (`min-content`, `max-content`, `fit-content`, `fit-content(...)`,
/// and `stretch`) on the size styles of an absolutely positioned item, filling in the
/// corresponding `known_dimensions` axes.
///
/// - `area_size` is the size of the item's containing block (which insets and percentages
///   resolve against).
/// - The stretch size in each axis is the containing block minus the item's insets and margins
///   in that axis.
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_absolute_sizing_keywords(
    tree: &mut impl LayoutPartialTree,
    node: NodeId,
    known_dimensions: &mut Size<Option<f32>>,
    size_style: Size<Dimension>,
    area_size: Size<f32>,
    inset: Rect<Option<f32>>,
    margin: Rect<Option<f32>>,
    sizing_mode: SizingMode,
) {
    let stretch_size = absolute_stretch_size(area_size, inset, margin);

    let keyword_width = if known_dimensions.width.is_none() {
        resolve_sizing_keyword(size_style.width, Some(stretch_size.width), Some(area_size.width))
    } else {
        None
    };
    let keyword_height = if known_dimensions.height.is_none() {
        resolve_sizing_keyword(size_style.height, Some(stretch_size.height), Some(area_size.height))
    } else {
        None
    };

    match (keyword_width, keyword_height) {
        // If both axes need to be measured then resolve them with a single measure call
        (
            Some(SizingKeywordResolution::Measure(available_width)),
            Some(SizingKeywordResolution::Measure(available_height)),
        ) => {
            let measured_size = tree.measure_child_size_both(
                node,
                Size::NONE,
                area_size.map(Some),
                Size { width: available_width, height: available_height },
                sizing_mode,
                Line::FALSE,
            );
            *known_dimensions = measured_size.map(Some);
        }
        (keyword_width, keyword_height) => {
            if let Some(resolution) = keyword_width {
                known_dimensions.width = Some(match resolution {
                    SizingKeywordResolution::Exact(width) => width,
                    SizingKeywordResolution::Measure(available_width) => tree.measure_child_size(
                        node,
                        *known_dimensions,
                        area_size.map(Some),
                        Size { width: available_width, height: AvailableSpace::Definite(stretch_size.height) },
                        sizing_mode,
                        AbsoluteAxis::Horizontal,
                        Line::FALSE,
                    ),
                });
            }
            if let Some(resolution) = keyword_height {
                known_dimensions.height = Some(match resolution {
                    SizingKeywordResolution::Exact(height) => height,
                    SizingKeywordResolution::Measure(available_height) => tree.measure_child_size(
                        node,
                        *known_dimensions,
                        area_size.map(Some),
                        Size {
                            width: known_dimensions
                                .width
                                .map(AvailableSpace::Definite)
                                .unwrap_or(AvailableSpace::Definite(stretch_size.width)),
                            height: available_height,
                        },
                        sizing_mode,
                        AbsoluteAxis::Vertical,
                        Line::FALSE,
                    ),
                });
            }
        }
    }
}

/// Resolve the sizing keywords on the min/max size styles of an absolutely positioned item,
/// overwriting the corresponding `min_size`/`max_size` axes (with `None` if a keyword cannot be
/// resolved). `size` is the item's definite style size, which fixes the other axis when
/// measuring; otherwise that axis gets the stretch size, as in [`resolve_absolute_sizing_keywords`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_absolute_min_max_sizing_keywords(
    tree: &mut impl LayoutPartialTree,
    node: NodeId,
    min_size: &mut Size<Option<f32>>,
    max_size: &mut Size<Option<f32>>,
    min_size_style: Size<Dimension>,
    max_size_style: Size<Dimension>,
    size: Size<Option<f32>>,
    area_size: Size<f32>,
    inset: Rect<Option<f32>>,
    margin: Rect<Option<f32>>,
) {
    let stretch_size = absolute_stretch_size(area_size, inset, margin);
    let available_space = stretch_size.map(AvailableSpace::Definite);
    let mut resolve = |style: Dimension, axis: AbsoluteAxis| {
        resolve_min_max_sizing_keyword(
            tree,
            node,
            style,
            axis,
            size,
            area_size.map(Some),
            available_space,
            Some(stretch_size.get_abs(axis)),
            Some(area_size.get_abs(axis)),
            Line::FALSE,
        )
    };
    if min_size_style.width.is_sizing_keyword() {
        min_size.width = resolve(min_size_style.width, AbsoluteAxis::Horizontal);
    }
    if max_size_style.width.is_sizing_keyword() {
        max_size.width = resolve(max_size_style.width, AbsoluteAxis::Horizontal);
    }
    if min_size_style.height.is_sizing_keyword() {
        min_size.height = resolve(min_size_style.height, AbsoluteAxis::Vertical);
    }
    if max_size_style.height.is_sizing_keyword() {
        max_size.height = resolve(max_size_style.height, AbsoluteAxis::Vertical);
    }
}

/// The size an absolutely positioned item would take if stretched: the containing block minus
/// the item's insets and margins
fn absolute_stretch_size(area_size: Size<f32>, inset: Rect<Option<f32>>, margin: Rect<Option<f32>>) -> Size<f32> {
    Size {
        width: f32_max(
            area_size.width
                - inset.left.unwrap_or(0.0)
                - inset.right.unwrap_or(0.0)
                - margin.left.unwrap_or(0.0)
                - margin.right.unwrap_or(0.0),
            0.0,
        ),
        height: f32_max(
            area_size.height
                - inset.top.unwrap_or(0.0)
                - inset.bottom.unwrap_or(0.0)
                - margin.top.unwrap_or(0.0)
                - margin.bottom.unwrap_or(0.0),
            0.0,
        ),
    }
}

#[cfg(all(test, feature = "taffy_tree", feature = "flexbox", feature = "block_layout"))]
mod min_max_keyword_tests {
    use crate::prelude::*;
    use crate::{AvailableSpace, Dimension, LayoutOutput};

    /// Text whose min-content width is 30 and max-content width 100; each 10px line holds up to
    /// the available width, so the content height is 10 per line.
    #[derive(Clone, Copy)]
    struct Text;

    fn measure(known: Size<Option<f32>>, available: Size<AvailableSpace>) -> Size<f32> {
        let width = known.width.unwrap_or(match available.width {
            AvailableSpace::MinContent => 30.0,
            AvailableSpace::MaxContent => 100.0,
            AvailableSpace::Definite(width) => width.clamp(30.0, 100.0),
        });
        let lines = (100.0 / width).ceil();
        Size { width, height: known.height.unwrap_or(lines * 10.0) }
    }

    struct Fixture {
        tree: TaffyTree<Text>,
        item: NodeId,
        root: NodeId,
    }

    impl Fixture {
        fn new(root_style: Style, item_style: Style) -> Self {
            let mut tree: TaffyTree<Text> = TaffyTree::new();
            let item = tree.new_leaf_with_context(item_style, Text).unwrap();
            let root = tree.new_with_children(root_style, &[item]).unwrap();
            Fixture { tree, item, root }
        }

        fn item_size(mut self) -> Size<f32> {
            self.tree
                .compute_layout_with_measure(self.root, Size::MAX_CONTENT, |inputs, _, ctx, _| {
                    LayoutOutput::from_outer_size(match ctx {
                        Some(Text) => measure(inputs.known_dimensions, inputs.available_space),
                        None => Size::ZERO,
                    })
                })
                .unwrap();
            self.tree.layout(self.item).unwrap().size
        }
    }

    fn container(display: Display, width: f32, height: f32) -> Style {
        Style {
            display,
            size: Size { width: length(width), height: length(height) },
            align_items: Some(AlignItems::START),
            ..Default::default()
        }
    }

    fn keyword(name: &str) -> Dimension {
        match name {
            "min" => Dimension::min_content(),
            "max" => Dimension::max_content(),
            "fit" => Dimension::fit_content(),
            _ => unreachable!(),
        }
    }

    // (keyword, container width, expected width) for `min-width` on a 10px-wide item
    const MIN_WIDTH_CASES: [(&str, f32, f32); 4] =
        [("min", 200.0, 30.0), ("max", 200.0, 100.0), ("fit", 60.0, 60.0), ("fit", 200.0, 100.0)];
    // (keyword, container width, expected width) for `max-width` on an item that would be 500px
    const MAX_WIDTH_CASES: [(&str, f32, f32); 4] =
        [("min", 200.0, 30.0), ("max", 200.0, 100.0), ("fit", 60.0, 60.0), ("fit", 200.0, 100.0)];

    fn width_cases(display: Display, position: Position) {
        for (name, container_width, expected) in MIN_WIDTH_CASES {
            let item = Style {
                position,
                flex_shrink: 0.0,
                size: Size { width: length(10.0), height: auto() },
                min_size: Size { width: keyword(name), height: auto() },
                ..Default::default()
            };
            let size = Fixture::new(container(display, container_width, 200.0), item).item_size();
            assert_eq!(size.width, expected, "{display:?} {position:?} min-width: {name} in {container_width}");
        }
        for (name, container_width, expected) in MAX_WIDTH_CASES {
            let item = Style {
                position,
                flex_shrink: 0.0,
                size: Size { width: length(500.0), height: auto() },
                max_size: Size { width: keyword(name), height: auto() },
                ..Default::default()
            };
            let size = Fixture::new(container(display, container_width, 200.0), item).item_size();
            assert_eq!(size.width, expected, "{display:?} {position:?} max-width: {name} in {container_width}");
        }
    }

    /// At a 50px width the text is 2 lines: every intrinsic height keyword is 20px
    fn height_cases(display: Display, position: Position) {
        for name in ["min", "max", "fit"] {
            let item = Style {
                position,
                flex_shrink: 0.0,
                size: Size { width: length(50.0), height: length(5.0) },
                min_size: Size { width: auto(), height: keyword(name) },
                ..Default::default()
            };
            let size = Fixture::new(container(display, 200.0, 200.0), item).item_size();
            assert_eq!(size.height, 20.0, "{display:?} {position:?} min-height: {name}");

            let item = Style {
                position,
                flex_shrink: 0.0,
                size: Size { width: length(50.0), height: length(500.0) },
                max_size: Size { width: auto(), height: keyword(name) },
                ..Default::default()
            };
            let size = Fixture::new(container(display, 200.0, 200.0), item).item_size();
            assert_eq!(size.height, 20.0, "{display:?} {position:?} max-height: {name}");
        }
    }

    #[test]
    fn flex_row_main_axis() {
        width_cases(Display::Flex, Position::Relative);
    }

    #[test]
    fn flex_row_cross_axis() {
        height_cases(Display::Flex, Position::Relative);
    }

    #[test]
    fn flex_column_main_axis() {
        let column = |style: Style| Style { flex_direction: FlexDirection::Column, ..style };
        for name in ["min", "max", "fit"] {
            let item = Style {
                flex_shrink: 0.0,
                size: Size { width: length(50.0), height: length(5.0) },
                min_size: Size { width: auto(), height: keyword(name) },
                ..Default::default()
            };
            let size = Fixture::new(column(container(Display::Flex, 200.0, 200.0)), item).item_size();
            assert_eq!(size.height, 20.0, "column min-height: {name}");

            let item = Style {
                flex_shrink: 0.0,
                size: Size { width: length(50.0), height: length(500.0) },
                max_size: Size { width: auto(), height: keyword(name) },
                ..Default::default()
            };
            let size = Fixture::new(column(container(Display::Flex, 200.0, 200.0)), item).item_size();
            assert_eq!(size.height, 20.0, "column max-height: {name}");
        }
    }

    #[test]
    fn flex_column_cross_axis() {
        let column = |style: Style| Style { flex_direction: FlexDirection::Column, ..style };
        let item = Style {
            size: Size { width: length(10.0), height: auto() },
            min_size: Size { width: Dimension::min_content(), height: auto() },
            ..Default::default()
        };
        assert_eq!(Fixture::new(column(container(Display::Flex, 200.0, 200.0)), item).item_size().width, 30.0);
        let item = Style {
            size: Size { width: length(500.0), height: auto() },
            max_size: Size { width: Dimension::max_content(), height: auto() },
            ..Default::default()
        };
        assert_eq!(Fixture::new(column(container(Display::Flex, 200.0, 200.0)), item).item_size().width, 100.0);
    }

    /// A stretched item is capped by its keyword max cross size
    #[test]
    fn flex_stretch_capped_by_keyword_max() {
        let root = Style { align_items: Some(AlignItems::STRETCH), ..container(Display::Flex, 200.0, 200.0) };
        let item = Style {
            size: Size { width: length(50.0), height: auto() },
            max_size: Size { width: auto(), height: Dimension::max_content() },
            ..Default::default()
        };
        assert_eq!(Fixture::new(root, item).item_size().height, 20.0);
    }

    /// A keyword minimum overrides the automatic minimum, and a flexible item grows up to a
    /// keyword maximum
    #[test]
    fn flex_grow_and_shrink_respect_keywords() {
        let item = Style {
            flex_grow: 1.0,
            max_size: Size { width: Dimension::min_content(), height: auto() },
            ..Default::default()
        };
        assert_eq!(Fixture::new(container(Display::Flex, 200.0, 200.0), item).item_size().width, 30.0);
        let item = Style {
            size: Size { width: length(500.0), height: auto() },
            min_size: Size { width: Dimension::max_content(), height: auto() },
            ..Default::default()
        };
        assert_eq!(Fixture::new(container(Display::Flex, 60.0, 200.0), item).item_size().width, 100.0);
    }

    #[test]
    fn block_width() {
        width_cases(Display::Block, Position::Relative);
    }

    #[test]
    fn block_height() {
        height_cases(Display::Block, Position::Relative);
    }

    /// An auto-width block item stretches, but no wider than its keyword maximum
    #[test]
    fn block_auto_width_capped_by_keyword_max() {
        for (name, expected) in [("min", 30.0), ("max", 100.0), ("fit", 100.0)] {
            let item = Style { max_size: Size { width: keyword(name), height: auto() }, ..Default::default() };
            let size = Fixture::new(container(Display::Block, 200.0, 200.0), item).item_size();
            assert_eq!(size.width, expected, "block auto width, max-width: {name}");
        }
    }

    /// A shrink-to-fit block container's width includes an item's keyword minimum
    #[test]
    fn block_intrinsic_width_contribution() {
        let item = Style {
            size: Size { width: length(10.0), height: auto() },
            min_size: Size { width: Dimension::max_content(), height: auto() },
            ..Default::default()
        };
        let mut tree: TaffyTree<Text> = TaffyTree::new();
        let item = tree.new_leaf_with_context(item, Text).unwrap();
        let block = tree.new_with_children(Style { display: Display::Block, ..Default::default() }, &[item]).unwrap();
        let root = tree
            .new_with_children(
                Style { display: Display::Flex, align_items: Some(AlignItems::START), ..Default::default() },
                &[block],
            )
            .unwrap();
        tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |inputs, _, ctx, _| {
            LayoutOutput::from_outer_size(match ctx {
                Some(Text) => measure(inputs.known_dimensions, inputs.available_space),
                None => Size::ZERO,
            })
        })
        .unwrap();
        assert_eq!(tree.layout(block).unwrap().size.width, 100.0);
        assert_eq!(tree.layout(item).unwrap().size.width, 100.0);
    }

    #[test]
    fn absolute_in_flex() {
        width_cases(Display::Flex, Position::Absolute);
        height_cases(Display::Flex, Position::Absolute);
    }

    #[test]
    fn absolute_in_block() {
        width_cases(Display::Block, Position::Absolute);
        height_cases(Display::Block, Position::Absolute);
    }

    /// Grid does not resolve keyword min/max sizes yet: a keyword minimum behaves as `auto` and
    /// a keyword maximum as `none`
    #[cfg(feature = "grid")]
    #[test]
    fn grid_ignores_keywords_without_panicking() {
        for name in ["min", "max", "fit"] {
            let item = Style {
                size: Size { width: length(10.0), height: auto() },
                min_size: Size { width: keyword(name), height: keyword(name) },
                max_size: Size { width: keyword(name), height: keyword(name) },
                ..Default::default()
            };
            let size = Fixture::new(container(Display::Grid, 200.0, 200.0), item).item_size();
            assert_eq!(size.width, 10.0, "grid min/max-width: {name}");
        }
    }
}
