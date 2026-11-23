//! Review probes (not to be committed).
use super::tests::app;
use super::*;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name)
}

/// The shipped test's pan assertion (`pan.0 < 0 && pan.1 < 0`) already holds before any drag.
#[test]
fn probe_pan_assertion_holds_without_a_drag() {
    let mut a = app(&["linked.md"]);
    let raster = Rc::new(mdview_render::DecodedImage {
        width: 2400,
        height: 1200,
        baseline: 1200,
        rgba: vec![200; 2400 * 1200 * 4],
    });
    a.show_zoom(raster);
    a.compose();
    let area = a.zoom_area();
    a.ui.mouse = (area.cx(), area.cy());
    a.wheel(MouseScrollDelta::LineDelta(0.0, 1.0));
    let view = a.ui.zoom.as_ref().unwrap();
    eprintln!("PROBE pan after the wheel, no drag: {:?}", view.pan);
    assert!(view.pan.0 < 0.0 && view.pan.1 < 0.0);
}

/// What each pass of the loop costs while a tab the editor asked for is open.
#[test]
fn probe_editor_closed_cost_per_loop_turn() {
    let mut a = app(&["tables.md"]);
    let n = 5000;
    let t = std::time::Instant::now();
    for _ in 0..n {
        assert!(a.editor_closed().is_empty());
    }
    let none = t.elapsed();
    for f in ["showcase.md", "math.md", "linked.md"] {
        a.handed_over(vec![Request {
            from: Some("mdedit".into()),
            ..Request::new(&fixture(f))
        }]);
    }
    let t = std::time::Instant::now();
    for _ in 0..n {
        assert!(a.editor_closed().is_empty());
    }
    let three = t.elapsed();
    eprintln!(
        "PROBE editor_closed per call: none asked {:.2} us, three asked {:.2} us",
        none.as_secs_f64() * 1e6 / n as f64,
        three.as_secs_f64() * 1e6 / n as f64
    );
}

/// The zoom view at a device scale of 2: how large it opens against the page's own drawing.
#[test]
fn probe_zoom_size_at_scale_two() {
    for scale in [1.0f32, 1.5, 2.0] {
        let mut a = app(&["linked.md"]);
        a.scale = scale;
        a.size = ((1280.0 * scale) as u32, (800.0 * scale) as u32);
        a.compose();
        // A raster the page made at 2 * scale of a diagram 500 logical px wide.
        let w = (500.0 * 2.0 * scale) as u32;
        let raster = Rc::new(mdview_render::DecodedImage {
            width: w,
            height: 100,
            baseline: 100,
            rgba: vec![200; w as usize * 100 * 4],
        });
        a.show_zoom(raster);
        let r = a.ui.zoom.as_ref().unwrap().image_rect(a.zoom_area());
        // The page draws it image.width / 2 device px wide = 500 logical px.
        eprintln!(
            "PROBE scale {scale}: page shows 500 logical px, zoom opens at {} logical px ({}x), each raster px on {} device px",
            r.w,
            r.w / 500.0,
            scale
        );
    }
}

/// The view stays up while the document under it changes.
#[test]
fn probe_zoom_stays_over_another_tab() {
    let mut a = app(&["linked.md"]);
    let raster = Rc::new(mdview_render::DecodedImage {
        width: 300,
        height: 200,
        baseline: 200,
        rgba: vec![200; 300 * 200 * 4],
    });
    a.show_zoom(raster);
    a.compose();
    a.handed_over(vec![Request {
        from: Some("mdedit".into()),
        ..Request::new(&fixture("math.md"))
    }]);
    a.compose();
    eprintln!(
        "PROBE after the editor opened math.md: zoom open = {}, active tab = {:?}",
        a.ui.zoom.is_some(),
        a.ws.active().map(|i| a.ws.labels()[i].clone())
    );
    // The window's own close button cannot be hit while the view is up.
    let (w, _) = a.logical();
    eprintln!("PROBE hit at the window's close button: {:?}", a.hit_at(w - 20.0, 14.0));
    eprintln!("PROBE hit at the window's title bar: {:?}", a.hit_at(w / 2.0, 14.0));
}
