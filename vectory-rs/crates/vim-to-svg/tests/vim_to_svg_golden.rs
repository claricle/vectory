//! Golden tests: VIM drawings → SVG bytes through the EmfsvgSvgBuilder
//! profile. Each golden encodes the reference renderer's byte rules for one
//! element family (declared in the crate docs): the declaration literal, the
//! preamble, `%.4f` numbers, attribute orders per arrangement, and the
//! trailing newline discipline.

use vector_model::{
    Arrangement, BaselineMode, Canvas, ClipDef, ClipShape, Color, CoordJoin, Dash, Draw, DrawBody,
    Drawing, Fill, HatchPatternDef, HatchStyle, ImageArrangement, ImageDraw, Item, LineCap,
    LineJoin, Matrix, PathDraw, PathGeometry, PathPaint, Preamble, Raster, RasterFormat,
    RasterPatternDef, Segment, ShapeDraw, ShapeKind, Stroke, TextAnchor, TextDraw, Width,
};
use vim_to_svg::drawing_to_svg;

fn canvas(width: f64, height: f64) -> Canvas {
    Canvas {
        width,
        height,
        preamble: Preamble { tx: 0.0, ty: 0.0 },
        degenerate_y_repair: false,
        alias_namespace: false,
    }
}

fn solid_paint(fill: Option<Fill>, stroke: Option<Stroke>) -> PathPaint {
    PathPaint {
        fill,
        stroke,
        clip: None,
        stroke_from_fill: false,
    }
}

#[test]
fn minimal_drawing_emits_the_reference_skeleton() {
    let mut drawing = Drawing::new(canvas(100.0, 50.0));
    drawing.items.push(Item::Draw(Draw {
        body: DrawBody::Shape(ShapeDraw {
            kind: ShapeKind::Rect {
                x: 10.0,
                y: 20.0,
                width: 30.5,
                height: 40.25,
            },
        }),
        paint: solid_paint(Some(Fill::solid(Color::new(0x12, 0x34, 0x56))), None),
        arrangement: Arrangement::RegionFillFirst,
    }));
    let svg = drawing_to_svg(&drawing);
    assert_eq!(
        svg,
        concat!(
            "<?xml version=\"1.0\"  encoding=\"UTF-8\" standalone=\"no\"?>\n",
            "<svg version=\"1.1\" xmlns=\"http://www.w3.org/2000/svg\" ",
            "xmlns:xlink=\"http://www.w3.org/1999/xlink\" ",
            "width=\"100.0000\" height=\"50.0000\">\n",
            "<g transform=\"translate(0.0000, 0.0000)\">\n",
            "<rect x=\"10.0000\" y=\"20.0000\" width=\"30.5000\" height=\"40.2500\" ",
            "fill=\"#123456\" stroke=\"none\"/>\n",
            "</g>\n",
            "</svg>\n"
        )
    );
}

#[test]
fn deferred_path_close_fill_stroke_fallbacks() {
    let mut drawing = Drawing::new(canvas(10.0, 10.0));
    let mut geometry = PathGeometry::new(CoordJoin::Tuple);
    geometry.push(Segment::MoveTo { x: 1.5, y: 2.5 });
    geometry.push(Segment::LineTo { x: 3.0, y: 4.0 });
    geometry.push(Segment::Close);
    drawing.items.push(Item::Draw(Draw {
        body: DrawBody::Path(PathDraw { geometry }),
        paint: PathPaint {
            fill: Some(Fill::solid(Color::new(0, 0, 0))),
            stroke: Some(Stroke::Solid {
                color: Color::new(255, 0, 0),
                width: Width::Scaled(2.5),
                dash: None,
                cap: Some(LineCap::Round),
                join: Some(LineJoin::Miter { limit: 10.0 }),
            }),
            clip: None,
            stroke_from_fill: false,
        },
        arrangement: Arrangement::DeferredPath,
    }));
    let svg = drawing_to_svg(&drawing);
    let path_line = svg
        .lines()
        .find(|l| l.starts_with("<path"))
        .expect("path element");
    // Deferred close paints fill first, then stroke (color, width, cap, join
    // with miter limit), then no fallbacks.
    assert_eq!(
        path_line,
        "<path d=\"M 1.5000,2.5000 L 3.0000,4.0000 Z \" fill=\"#000000\" \
         stroke=\"#FF0000\" stroke-width=\"2.5000\" stroke-linecap=\"round\" \
         stroke-linejoin=\"miter\" stroke-miterlimit=\"10.0000\"/>"
    );
}

#[test]
fn contour_open_falls_back_to_unfilled() {
    let mut drawing = Drawing::new(canvas(10.0, 10.0));
    let mut geometry = PathGeometry::new(CoordJoin::Tuple);
    geometry.push(Segment::MoveTo { x: 0.0, y: 0.0 });
    geometry.push(Segment::LineTo { x: 5.0, y: 5.0 });
    drawing.items.push(Item::Draw(Draw {
        body: DrawBody::Path(PathDraw { geometry }),
        paint: solid_paint(
            None,
            Some(Stroke::Solid {
                color: Color::new(0, 128, 0),
                width: Width::OnePixel,
                dash: Some(Dash(vec![5.0, 5.0])),
                cap: None,
                join: None,
            }),
        ),
        arrangement: Arrangement::ContourOpen {
            leading_fill_space: true,
        },
    }));
    let svg = drawing_to_svg(&drawing);
    let path_line = svg
        .lines()
        .find(|l| l.starts_with("<path"))
        .expect("path element");
    // Stroke drawn, so only the fill fallback applies (no stroke fallback).
    assert_eq!(
        path_line,
        "<path d=\"M 0.0000,0.0000 L 5.0000,5.0000 \" stroke=\"#008000\" \
         stroke-width=\"1px\" stroke-dasharray=\"5.0000,5.0000\" fill=\"none\"/>"
    );
}

#[test]
fn attr_map_path_carries_geometry_last_without_newline() {
    let mut drawing = Drawing::new(canvas(10.0, 10.0));
    let mut geometry = PathGeometry::new(CoordJoin::Sequence);
    geometry.push(Segment::MoveTo { x: 1.0, y: 2.0 });
    geometry.push(Segment::LineTo { x: 3.0, y: 4.0 });
    drawing.items.push(Item::Draw(Draw {
        body: DrawBody::Path(PathDraw { geometry }),
        paint: solid_paint(
            Some(Fill::solid(Color::new(0, 0, 255))),
            Some(Stroke::Solid {
                color: Color::new(0, 0, 0),
                width: Width::Count(2),
                dash: None,
                cap: Some(LineCap::Butt),
                join: None,
            }),
        ),
        arrangement: Arrangement::AttrMap,
    }));
    drawing.items.push(Item::Draw(Draw {
        body: DrawBody::Shape(ShapeDraw {
            kind: ShapeKind::Ellipse {
                cx: 5.0,
                cy: 5.0,
                rx: 2.0,
                ry: 2.0,
            },
        }),
        paint: solid_paint(Some(Fill::none()), None),
        arrangement: Arrangement::RegionFillFirst,
    }));
    let svg = drawing_to_svg(&drawing);
    let path_line = svg
        .lines()
        .find(|l| l.starts_with("<path"))
        .expect("path element");
    // Attribute-map order: fill, stroke, stroke-width (raw count), cap, d
    // last; no newline follows, so the next element continues on the line.
    assert!(
        path_line.starts_with(
            "<path fill=\"#0000FF\" stroke=\"#000000\" stroke-width=\"2\" \
             stroke-linecap=\"butt\" d=\"M 1.0000 2.0000 L 3.0000 4.0000 \"/>"
        ),
        "{path_line}"
    );
}

#[test]
fn clip_def_and_reference() {
    let mut drawing = Drawing::new(canvas(20.0, 20.0));
    let mut geometry = PathGeometry::new(CoordJoin::Tuple);
    geometry.push(Segment::MoveTo { x: 0.0, y: 0.0 });
    geometry.push(Segment::LineTo { x: 10.0, y: 0.0 });
    geometry.push(Segment::LineTo { x: 10.0, y: 10.0 });
    geometry.push(Segment::LineTo { x: 0.0, y: 10.0 });
    geometry.push(Segment::LineTo { x: 0.0, y: 0.0 });
    geometry.push(Segment::Close);
    drawing.items.push(Item::DefineClip(ClipDef {
        id: 16807,
        shape: ClipShape::Bounds(geometry),
    }));
    drawing.items.push(Item::Draw(Draw {
        body: DrawBody::Shape(ShapeDraw {
            kind: ShapeKind::Rect {
                x: 1.0,
                y: 1.0,
                width: 5.0,
                height: 5.0,
            },
        }),
        paint: PathPaint {
            fill: Some(Fill::solid(Color::new(0, 0, 0))),
            stroke: None,
            clip: Some(16807),
            stroke_from_fill: false,
        },
        arrangement: Arrangement::RegionFillFirst,
    }));
    let svg = drawing_to_svg(&drawing);
    // The bounds geometry carries its Close segment, and the clip def appends
    // its own Z (the reference's doubled close, byte-reproduced); the
    // space-before-tag-close quirk is not representable in the SVG model.
    assert!(svg.contains(
        "<defs><clipPath id=\"clip-16807\"><path d=\"M 0.0000,0.0000 L 10.0000,0.0000 \
         L 10.0000,10.0000 L 0.0000,10.0000 L 0.0000,0.0000 Z Z\"/></clipPath></defs>"
    ));
    assert!(svg.contains("clip-path=\"url(#clip-16807)\""));
}

#[test]
fn text_run_carries_the_attribute_sequence() {
    let mut drawing = Drawing::new(canvas(50.0, 20.0));
    drawing.items.push(Item::Draw(Draw {
        body: DrawBody::Text(TextDraw {
            x: 10.0,
            y: 15.0,
            font_height: 12.0,
            face: Some("Arial".to_string()),
            color: Color::new(0, 0, 0),
            rotation: Some(-270),
            italic: true,
            underline: false,
            strikeout: true,
            weight: 700,
            anchor: TextAnchor::Middle,
            baseline: BaselineMode::Raised,
            content: "hi".to_string(),
        }),
        paint: solid_paint(None, None),
        arrangement: Arrangement::RegionFillFirst,
    }));
    let svg = drawing_to_svg(&drawing);
    let text_line = svg
        .lines()
        .find(|l| l.starts_with("<text"))
        .expect("text element");
    // raise = 12.0 * 0.9 = 10.8; both the raised y and the transform anchor
    // use it (%.4f: "10.8000", "25.8000").
    assert_eq!(
        text_line,
        "<text font-family=\"Arial\" fill=\"#000000\" transform=\"rotate(-270, 10.0000, \
         25.8000) translate(0, 10.8000)\" font-style=\"italic\" style=\"white-space:pre;\" \
         text-decoration=\"line-through\" font-weight=\"700\" text-anchor=\"middle\" \
         x=\"10.0000\" y=\"25.8000\" font-size=\"12.0000\"><![CDATA[hi]]></text>"
    );
}

#[test]
fn raster_pattern_def_and_calibrated_image() {
    let mut drawing = Drawing::new(canvas(30.0, 30.0));
    drawing
        .items
        .push(Item::DefineRasterPattern(RasterPatternDef {
            id: 1,
            raster: Raster {
                format: RasterFormat::Png,
                payload: b"PNG!".to_vec(),
            },
            width: 4,
            height: 3,
        }));
    drawing.items.push(Item::Draw(Draw {
        body: DrawBody::Image(ImageDraw {
            raster: Raster {
                format: RasterFormat::Png,
                payload: b"PNG!".to_vec(),
            },
            arrangement: ImageArrangement::Calibrated {
                width: 8.0,
                height: 6.0,
                x: 1.0,
                y: 2.0,
            },
        }),
        paint: solid_paint(None, None),
        arrangement: Arrangement::RegionFillFirst,
    }));
    let svg = drawing_to_svg(&drawing);
    assert!(svg.contains("<defs><image id=\"img-1\" x=\"0\" y=\"0\" width=\"4\" height=\"3\" "));
    assert!(svg.contains("xlink:href=\"data:image/png;base64,UE5HIQ==\""));
    assert!(svg.contains("<pattern id=\"img-1-ref\" x=\"0\" y=\"0\" width=\"4\" height=\"3\" "));
    assert!(svg.contains("patternUnits=\"userSpaceOnUse\">"));
    assert!(svg.contains("<use id=\"img-1-ign\" xlink:href=\"#img-1\"/></pattern></defs>"));
    let image_line = svg
        .lines()
        .find(|l| l.starts_with("<image"))
        .expect("image element");
    assert_eq!(
        image_line,
        "<image width=\"8.0000\" height=\"6.0000\" x=\"1.0000\" y=\"2.0000\" \
         xlink:href=\"data:image/png;base64,UE5HIQ==\"/>"
    );
}

#[test]
fn hatch_pattern_and_group_transforms() {
    let mut drawing = Drawing::new(canvas(10.0, 10.0));
    drawing
        .items
        .push(Item::DefineHatchPattern(HatchPatternDef {
            id: "hatch-1".to_string(),
            style: HatchStyle::Cross,
            foreground: Color::new(0, 0, 0),
            background: Some(Color::new(255, 255, 255)),
        }));
    drawing.items.push(Item::GroupOpen {
        transform: Matrix {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            dx: 5.0,
            dy: -6.0,
        },
    });
    drawing.items.push(Item::Draw(Draw {
        body: DrawBody::Shape(ShapeDraw {
            kind: ShapeKind::RoundedRect {
                x: 0.0,
                y: 0.0,
                width: 4.0,
                height: 4.0,
                rx: 1.0,
                ry: 2.0,
            },
        }),
        paint: solid_paint(
            None,
            Some(Stroke::Solid {
                color: Color::new(0, 0, 0),
                width: Width::Scaled(1.0),
                dash: None,
                cap: None,
                join: Some(LineJoin::Bevel),
            }),
        ),
        arrangement: Arrangement::RegionFillFirst,
    }));
    drawing.items.push(Item::GroupClose);
    let svg = drawing_to_svg(&drawing);
    assert!(svg.contains(
        "<defs><pattern id=\"hatch-1\" width=\"8\" height=\"8\" \
         patternUnits=\"userSpaceOnUse\"><rect width=\"8\" height=\"8\" \
         fill=\"#FFFFFF\"/><path d=\"M0,4 L8,4\" stroke=\"#000000\" \
         stroke-width=\"1\"/><path d=\"M4,0 L4,8\" stroke=\"#000000\" \
         stroke-width=\"1\"/></pattern></defs>"
    ));
    assert!(svg.contains("<g transform=\"matrix(1.0000 0.0000 0.0000 1.0000 5.0000 -6.0000)\">"));
    assert!(
        svg.contains(
            "<rect x=\"0.0000\" y=\"0.0000\" width=\"4.0000\" height=\"4.0000\" \
        rx=\"1.0000\" ry=\"2.0000\" stroke=\"#000000\" stroke-width=\"1.0000\" \
        stroke-linejoin=\"bevel\" fill=\"none\"/>"
        ),
        "{svg}"
    );
}

#[test]
fn degenerate_y_repair_emits_the_literal_zeroed_preamble() {
    let mut canvas = canvas(472.5, 527.5);
    canvas.degenerate_y_repair = true;
    let drawing = Drawing::new(canvas);
    let svg = drawing_to_svg(&drawing);
    assert!(svg.contains("<g transform=\"translate(0.0000, 0.00 00)\">"));
    assert!(svg.contains("width=\"473.5000\" height=\"528.5000\">"));
}

#[test]
fn solid_rect_fill_and_bare_shape_continue_in_stream() {
    let mut drawing = Drawing::new(canvas(10.0, 10.0));
    let mut geometry = PathGeometry::new(CoordJoin::Tuple);
    geometry.push(Segment::MoveTo { x: 0.0, y: 0.0 });
    geometry.push(Segment::LineTo { x: 4.0, y: 0.0 });
    geometry.push(Segment::LineTo { x: 4.0, y: 4.0 });
    geometry.push(Segment::LineTo { x: 0.0, y: 4.0 });
    geometry.push(Segment::Close);
    drawing.items.push(Item::Draw(Draw {
        body: DrawBody::Path(PathDraw { geometry }),
        paint: solid_paint(Some(Fill::solid(Color::new(0xAB, 0xCD, 0xEF))), None),
        arrangement: Arrangement::SolidRectFill,
    }));
    drawing.items.push(Item::Draw(Draw {
        body: DrawBody::BareRect(vector_model::BareRect {
            x: "1".to_string(),
            y: "2".to_string(),
            width: "3.5".to_string(),
            height: "4".to_string(),
            fill: "#123456".to_string(),
        }),
        paint: solid_paint(None, None),
        arrangement: Arrangement::BareShape,
    }));
    let svg = drawing_to_svg(&drawing);
    // Neither family emits a newline: they run together on one line.
    assert!(svg.contains(
        "<path style=\"fill:#ABCDEF\" d=\"M 0.0000,0.0000 L 4.0000,0.0000 L 4.0000,4.0000 \
         L 0.0000,4.0000 Z \"/><rect x=\"1\" y=\"2\" width=\"3.5\" height=\"4\" \
         fill=\"#123456\"/>"
    ));
}
