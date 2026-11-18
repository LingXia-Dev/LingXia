fn laid_out() -> crate::DisplayList {
    let module = crate::session::tests::source(
        "function render(){return h('view',{style:{flexGrow:1,backgroundColor:'#ffffff'}},h('text',null,'Sharp'))}",
    );
    crate::ViewSession::new(&module)
        .unwrap()
        .layout(1179, 2556, 3.0)
        .unwrap()
}

/// iPhone 15 Pro (393 x 852 pt at 3x) shown on a 1x screen, then shrunk to fit a smaller one.
#[test]
fn a_list_rasterizes_at_the_shown_scale_and_keeps_the_devices_for_a_capture() {
    let mut display = laid_out();
    let paints = display.commands.clone();
    display.raster_at(1.0);
    assert_eq!(
        (display.width, display.height, display.scale),
        (393, 852, 1.0)
    );
    assert_eq!(display.device_raster(), Some((1179, 2556, 3.0)));
    assert!(display.commands == paints, "the paints stay as laid out");
    // A new shown scale starts from the device's, not from the last one shown.
    display.raster_at(0.82);
    assert_eq!((display.width, display.height), (322, 699));
    assert_eq!(display.device_raster(), Some((1179, 2556, 3.0)));
    #[cfg(feature = "gpu")]
    {
        display.swap_raster();
        assert_eq!(
            (display.width, display.height, display.scale),
            (1179, 2556, 3.0)
        );
        display.swap_raster();
        assert_eq!(
            (display.width, display.height, display.scale),
            (322, 699, 0.82)
        );
    }
    display.raster_at(3.0);
    assert_eq!(
        (display.width, display.height, display.scale),
        (1179, 2556, 3.0)
    );
    assert_eq!(display.device_raster(), None);
    display.raster_at(f32::NAN);
    assert_eq!(
        (display.width, display.height, display.scale),
        (1179, 2556, 3.0)
    );
}
