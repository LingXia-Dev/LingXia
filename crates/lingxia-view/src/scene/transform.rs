//! Static CSS `translate`, `rotate`, `scale` and `transform-origin`. Translation
//! moves layout geometry (exact hits, editors and scrolls keep working);
//! rotation and scale are a paint affine about the origin.
use serde_json::Value;
use vello_cpu::kurbo::Affine;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct StaticTransform {
    /// Logical pixels, or a fraction of the box when `relative`.
    translate: [(f64, bool); 2],
    rotate: f64,
    scale: [f64; 2],
    origin: [f64; 2],
}

fn number(value: &Value, bound: f64) -> Option<f64> {
    value.as_f64().filter(|v| v.is_finite() && v.abs() <= bound)
}

fn length(value: &Value) -> Option<(f64, bool)> {
    match value.as_str().and_then(|text| text.strip_suffix('%')) {
        Some(text) => text
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite() && v.abs() <= 1000.0)
            .map(|v| (v / 100.0, true)),
        None => number(value, 16384.0).map(|v| (v, false)),
    }
}

fn pair<T: Copy>(value: &Value, item: impl Fn(&Value) -> Option<T>) -> Option<[T; 2]> {
    match value.as_array()?.as_slice() {
        [a, b] => Some([item(a)?, item(b)?]),
        _ => None,
    }
}

impl StaticTransform {
    pub fn parse(source: &str) -> Result<Self, String> {
        let invalid = || "UI_TRANSFORM: invalid transform".to_string();
        if source.len() > 512 {
            return Err(invalid());
        }
        let value: Value = serde_json::from_str(source).map_err(|_| invalid())?;
        let object = value.as_object().ok_or_else(invalid)?;
        let mut transform = Self {
            scale: [1.0; 2],
            origin: [0.5; 2],
            ..Self::default()
        };
        for (key, value) in object {
            match key.as_str() {
                "translate" => transform.translate = pair(value, length).ok_or_else(invalid)?,
                "rotate" => transform.rotate = number(value, 36000.0).ok_or_else(invalid)?,
                "scale" => {
                    transform.scale = pair(value, |v| number(v, 1000.0)).ok_or_else(invalid)?
                }
                "origin" => {
                    transform.origin = pair(value, |v| number(v, 16.0)).ok_or_else(invalid)?
                }
                _ => return Err(invalid()),
            }
        }
        Ok(transform)
    }

    /// The layout offset for a box of `size`.
    pub fn translation(&self, [w, h]: [f32; 2]) -> [f32; 2] {
        let [(x, rx), (y, ry)] = self.translate;
        [
            if rx { x * f64::from(w) } else { x },
            if ry { y * f64::from(h) } else { y },
        ]
        .map(|v| v as f32)
    }

    /// A zero scale paints nothing and takes no hits, but keeps its layout box.
    pub fn collapsed(&self) -> bool {
        self.scale.iter().any(|s| s.abs() < 1e-6)
    }

    /// Whether rotation or scale leave anything to paint as an affine.
    pub fn paints(&self) -> bool {
        self.rotate.rem_euclid(360.0) != 0.0 || self.scale != [1.0; 2]
    }

    /// Independent parts for transitions, translation resolved against `size`.
    pub fn decomposed(&self, size: [f32; 2]) -> crate::animation::transition::Decomposed {
        let [x, y] = self.translation(size);
        crate::animation::transition::Decomposed {
            translate: [f64::from(x), f64::from(y)],
            rotate: self.rotate,
            scale: self.scale,
            origin: self.origin,
        }
    }

    /// The paint affine about the origin of `bounds`.
    pub fn matrix(&self, [x, y, w, h]: [f32; 4]) -> Affine {
        let [x, y, w, h] = [x, y, w, h].map(f64::from);
        let origin = (x + self.origin[0] * w, y + self.origin[1] * h);
        Affine::translate(origin)
            * Affine::rotate(self.rotate.to_radians())
            * Affine::scale_non_uniform(self.scale[0], self.scale[1])
            * Affine::translate((-origin.0, -origin.1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bounded_parts_and_rejects_the_rest() {
        let t = StaticTransform::parse(
            r#"{"translate":[4,"-50%"],"rotate":90,"scale":[-1,1],"origin":[0,0]}"#,
        )
        .unwrap();
        assert_eq!(t.translation([10.0, 20.0]), [4.0, -10.0]);
        let m = t.matrix([10.0, 10.0, 20.0, 20.0]);
        let p = m * vello_cpu::kurbo::Point::new(20.0, 10.0);
        // Mirror, then a quarter turn clockwise, about the top-left origin.
        assert!((p.x - 10.0).abs() < 1e-9 && p.y.abs() < 1e-9, "{p:?}");
        assert!(t.paints() && !t.collapsed());
        assert!(
            StaticTransform::parse(r#"{"scale":[0,1]}"#)
                .unwrap()
                .collapsed()
        );
        assert!(
            !StaticTransform::parse(r#"{"translate":[1,2],"rotate":360}"#)
                .unwrap()
                .paints()
        );
        for source in [
            r#"{"skew":1}"#,
            r#"{"rotate":1e9}"#,
            r#"{"translate":[1]}"#,
            r#"{"translate":["2000%",0]}"#,
            r#"{"scale":[1,1e4]}"#,
            r#"[]"#,
        ] {
            assert!(StaticTransform::parse(source).is_err(), "{source}");
        }
    }

    fn page(render: &str) -> crate::ViewSession {
        crate::ViewSession::new(&crate::session::tests::source(&format!(
            "function render(){{return {render}}}"
        )))
        .unwrap()
    }

    fn node<'a>(display: &'a crate::DisplayList, id: &str) -> &'a crate::scene::node::NodeSnapshot {
        display.nodes.iter().find(|n| n.test_id == id).unwrap()
    }

    #[test]
    fn translation_moves_geometry_hits_and_descendants_without_moving_siblings() {
        let mut page = page(
            "h('column',{},h('column',{'data-testid':'b',style:{width:40,height:20,translate:[10,'50%']}},h('text',{'data-testid':'t'},'Go')),h('column',{'data-testid':'next',style:{height:10}}))",
        );
        let display = page.layout(100, 100, 1.0).unwrap();
        assert_eq!(node(&display, "b").bounds, [10.0, 10.0, 40.0, 20.0]);
        assert!(node(&display, "t").bounds[0] >= 10.0);
        assert_eq!(node(&display, "next").bounds[1], 20.0);
    }

    #[test]
    fn rotation_paints_about_the_origin_and_hits_the_rotated_shape() {
        let mut page = page(
            "h('page',{style:{backgroundColor:'#ffffff'}},h('button',{'data-testid':'bar',onClick(){},style:{position:'absolute',left:10,top:45,width:80,height:10,padding:0,borderRadius:0,backgroundColor:'#000000',rotate:90}}))",
        );
        let display = page.layout(100, 100, 1.0).unwrap();
        let bar = node(&display, "bar");
        // A quarter turn about the center stands the bar upright.
        assert!(
            (bar.bounds[0] - 45.0).abs() < 0.01 && (bar.bounds[3] - 80.0).abs() < 0.01,
            "{:?}",
            bar.bounds
        );
        let pixels = display.rasterize().unwrap().pixels;
        let at = |x: usize, y: usize| pixels.data_as_u8_slice()[(y * 100 + x) * 4];
        assert_eq!(at(50, 15), 0);
        assert_eq!(at(20, 50), 255);
        let mut tilted = page_with_rotation(45.0);
        let display = tilted.layout(100, 100, 1.0).unwrap();
        let bar = node(&display, "bar");
        // The extent's corner lies outside the rotated bar.
        assert!(bar.clips.last().unwrap().contains([50.0, 50.0]));
        assert!(
            !bar.clips
                .last()
                .unwrap()
                .contains([bar.bounds[0] + 1.0, bar.bounds[1] + 1.0])
        );
    }

    fn page_with_rotation(degrees: f64) -> crate::ViewSession {
        page(&format!(
            "h('page',{{}},h('button',{{'data-testid':'bar',onClick(){{}},style:{{position:'absolute',left:10,top:45,width:80,height:10,padding:0,rotate:{degrees}}}}}))"
        ))
    }

    #[test]
    fn mirroring_flips_vectors_and_zero_scale_paints_nothing() {
        let mut mirrored = page(
            "h('vector',{viewBox:[0,0,2,1],paths:[{d:'M0 0h1v1H0z',fill:'#000000'}],style:{width:20,height:10,backgroundColor:'#ffffff',objectFit:'fill',scale:[-1,1]}})",
        );
        let pixels = mirrored
            .layout(20, 10, 1.0)
            .unwrap()
            .rasterize()
            .unwrap()
            .pixels;
        let at = |x: usize| pixels.data_as_u8_slice()[(5 * 20 + x) * 4];
        assert_eq!((at(3), at(16)), (255, 0));
        let mut collapsed = page(
            "h('page',{style:{backgroundColor:'#ffffff'}},h('button',{'data-testid':'b',onClick(){},style:{width:20,height:20,backgroundColor:'#000000',scale:0}},'x'))",
        );
        let display = collapsed.layout(40, 40, 1.0).unwrap();
        assert_eq!(node(&display, "b").on_click, None);
        let pixels = display.rasterize().unwrap().pixels;
        assert!(pixels.data_as_u8_slice().chunks(4).all(|p| p[0] > 200));
    }

    #[test]
    fn editors_and_scrolls_cannot_rotate_but_may_translate() {
        let mut rotated = page("h('column',{style:{rotate:10}},h('input',{value:''}))");
        assert!(
            rotated
                .layout(100, 100, 1.0)
                .err()
                .unwrap()
                .contains("UI_TRANSFORM")
        );
        let mut moved =
            page("h('column',{style:{translate:[5,5]}},h('input',{'data-testid':'i',value:''}))");
        let display = moved.layout(200, 100, 1.0).unwrap();
        assert_eq!(node(&display, "i").bounds[0], 5.0);
    }
}
