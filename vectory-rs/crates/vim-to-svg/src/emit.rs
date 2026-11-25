//! The VIM → SVG tree emitter: drawing items → `vectory::Document` nodes,
//! with the reference renderer's element families, attribute orders, and
//! number forms. The tree serializes through the SVG model's existing
//! `EmfsvgSvgBuilder` profile.

use vector_model::{
    Arrangement, BaselineMode, Canvas, ClipShape, CoordJoin, DrawBody, Drawing, HatchStyle,
    ImageArrangement, Item, LineCap, LineJoin, PathPaint, RasterFormat, Segment, ShapeKind, Stroke,
    TextAnchor, Width,
};
use vectory::{Document, Element, Node, Text, XmlDeclaration};

use crate::number::{
    append_fmt4, append_point_sequence, append_point_tuple, base64_encode, fmt4, ruby_float_to_s,
    ruby_int_to_s,
};

/// The literal reference declaration (double space preserved verbatim).
const DECLARATION: &str = "xml version=\"1.0\"  encoding=\"UTF-8\" standalone=\"no\"";

/// Transforms a VIM drawing into an SVG document.
#[must_use]
pub fn drawing_to_document(drawing: &Drawing) -> Document {
    let mut emitter = Emitter::new(&drawing.canvas);
    emitter.emit_items(&drawing.items);
    emitter.finish()
}

/// Transforms a VIM drawing straight to SVG bytes.
///
/// # Errors
///
/// Never fails for well-formed drawings; the error type is the SVG model
/// serializer's (unused here since the tree is built, not decoded).
#[must_use]
pub fn drawing_to_svg(drawing: &Drawing) -> String {
    vectory::serialize(
        &drawing_to_document(drawing),
        vectory::Profile::EmfsvgSvgBuilder,
    )
}

struct Emitter<'a> {
    canvas: &'a Canvas,
    root: Element,
    /// Open group elements, innermost last; the next node lands in the
    /// innermost open group, or the root when empty.
    containers: Vec<Element>,
}

impl<'a> Emitter<'a> {
    fn new(canvas: &'a Canvas) -> Self {
        let width = if canvas.degenerate_y_repair {
            canvas.width + 1.0
        } else {
            canvas.width
        };
        let height = if canvas.degenerate_y_repair {
            canvas.height + 1.0
        } else {
            canvas.height
        };
        let mut svg = Element::new("svg");
        svg.set_attribute("version", "1.1");
        svg.set_attribute("xmlns", "http://www.w3.org/2000/svg");
        svg.set_attribute("xmlns:xlink", "http://www.w3.org/1999/xlink");
        svg.set_attribute("width", &fmt4(width));
        svg.set_attribute("height", &fmt4(height));
        if canvas.alias_namespace {
            svg.set_attribute("xmlns:svg", "http://www.w3.org/2000/svg");
        }
        Emitter {
            canvas,
            root: svg,
            containers: Vec::new(),
        }
    }

    fn push_node(&mut self, node: Node) {
        let target = match self.containers.last_mut() {
            Some(group) => &mut group.children,
            None => &mut self.root.children,
        };
        target.push(node);
    }

    fn push_text(&mut self, content: &str) {
        self.push_node(Node::Text(Text(content.to_string())));
    }

    fn push_element(&mut self, element: Element) {
        self.push_node(Node::Element(element));
    }

    fn emit_items(&mut self, items: &[Item]) {
        // The root's open tag is followed by a newline.
        self.root.children.push(Node::Text(Text("\n".to_string())));
        // Preamble anchor group.
        let mut preamble = Element::new("g");
        if self.canvas.degenerate_y_repair {
            // The reference repair's literal form (typo preserved verbatim).
            preamble.set_attribute("transform", "translate(0.0000, 0.00 00)");
        } else {
            let mut value = String::from("translate(");
            append_fmt4(&mut value, self.canvas.preamble.tx);
            value.push_str(", ");
            append_fmt4(&mut value, self.canvas.preamble.ty);
            value.push(')');
            preamble.set_attribute("transform", &value);
        }
        preamble.children.push(Node::Text(Text("\n".to_string())));
        self.containers.push(preamble);

        for item in items {
            self.emit_item(item);
        }
        // Close the preamble group; its newline follows the group element.
        let preamble = self.containers.pop().expect("preamble container");
        self.push_element(preamble);
        self.push_text("\n");
    }

    fn finish(self) -> Document {
        // Root close newline.
        Document {
            prolog: vec![
                Node::Declaration(XmlDeclaration {
                    raw: DECLARATION.to_string(),
                }),
                Node::Text(Text("\n".to_string())),
            ],
            root: Some(self.root),
            epilog: vec![Node::Text(Text("\n".to_string()))],
        }
    }

    fn emit_item(&mut self, item: &Item) {
        match item {
            Item::GroupOpen { transform } => {
                let mut group = Element::new("g");
                let mut value = String::from("matrix(");
                append_fmt4(&mut value, transform.m11);
                value.push(' ');
                append_fmt4(&mut value, transform.m12);
                value.push(' ');
                append_fmt4(&mut value, transform.m21);
                value.push(' ');
                append_fmt4(&mut value, transform.m22);
                value.push(' ');
                append_fmt4(&mut value, transform.dx);
                value.push(' ');
                append_fmt4(&mut value, transform.dy);
                value.push(')');
                group.set_attribute("transform", &value);
                group.children.push(Node::Text(Text("\n".to_string())));
                // Becomes the current container; GroupClose lands it.
                self.containers.push(group);
            }
            Item::GroupClose => {
                // The closing element ends; its newline lands in the parent.
                let group = self.containers.pop().expect("GroupClose without GroupOpen");
                self.push_element(group);
                self.push_text("\n");
            }
            Item::DefineClip(clip) => self.emit_clip_def(clip.id, &clip.shape),
            Item::DefineRasterPattern(def) => {
                self.emit_raster_pattern(def.id, &def.raster, def.width, def.height);
            }
            Item::DefineHatchPattern(def) => {
                self.emit_hatch_pattern(&def.id, def.style, def.foreground, def.background);
            }
            Item::Draw(draw) => match &draw.body {
                DrawBody::Shape(shape) => {
                    self.emit_shape(shape.kind, &draw.paint, draw.arrangement)
                }
                DrawBody::Path(path) => {
                    self.emit_path(&path.geometry, &draw.paint, draw.arrangement);
                }
                DrawBody::Text(text) => self.emit_text(text, &draw.paint),
                DrawBody::Image(image) => self.emit_image(image, &draw.paint),
                DrawBody::BareRect(rect) => self.emit_bare_rect(rect),
            },
        }
    }

    fn emit_clip_def(&mut self, id: u32, shape: &ClipShape) {
        let geometry = match shape {
            ClipShape::Bounds(geometry) | ClipShape::Path(geometry) => geometry,
        };
        let mut clip_path = Element::new("clipPath");
        clip_path.set_attribute("id", &format!("clip-{id}"));
        let mut path = Element::new("path");
        let mut d = path_d(geometry);
        d.push('Z');
        path.set_attribute("d", &d);
        clip_path.children.push(Node::Element(path));
        let mut defs = Element::new("defs");
        defs.children.push(Node::Element(clip_path));
        self.push_element(defs);
        self.push_text("\n");
    }

    fn emit_raster_pattern(
        &mut self,
        id: u32,
        raster: &vector_model::Raster,
        width: i64,
        height: i64,
    ) {
        let b64 = base64_encode(&raster.payload);
        let mut image = Element::new("image");
        image.set_attribute("id", &format!("img-{id}"));
        image.set_attribute("x", "0");
        image.set_attribute("y", "0");
        image.set_attribute("width", &width.to_string());
        image.set_attribute("height", &height.to_string());
        image.set_attribute("xlink:href", &format!("data:image/png;base64,{b64}"));
        image.set_attribute("preserveAspectRatio", "none");
        let mut pattern = Element::new("pattern");
        pattern.set_attribute("id", &format!("img-{id}-ref"));
        pattern.set_attribute("x", "0");
        pattern.set_attribute("y", "0");
        pattern.set_attribute("width", &width.to_string());
        pattern.set_attribute("height", &height.to_string());
        pattern.set_attribute("patternUnits", "userSpaceOnUse");
        pattern.children.push(Node::Text(Text("\n".to_string())));
        let mut use_element = Element::new("use");
        use_element.set_attribute("id", &format!("img-{id}-ign"));
        use_element.set_attribute("xlink:href", &format!("#img-{id}"));
        pattern.children.push(Node::Element(use_element));
        let mut defs = Element::new("defs");
        defs.children.push(Node::Element(image));
        defs.children.push(Node::Element(pattern));
        self.push_element(defs);
        self.push_text("\n");
    }

    fn emit_hatch_pattern(
        &mut self,
        id: &str,
        style: HatchStyle,
        foreground: vector_model::Color,
        background: Option<vector_model::Color>,
    ) {
        let lines: &[&str] = match style {
            HatchStyle::Horizontal => &["M0,4 L8,4"],
            HatchStyle::Vertical => &["M4,0 L4,8"],
            HatchStyle::ForwardDiagonal => &["M0,8 L8,0"],
            HatchStyle::BackDiagonal => &["M0,0 L8,8"],
            HatchStyle::Cross => &["M0,4 L8,4", "M4,0 L4,8"],
            HatchStyle::DiagonalCross => &["M0,8 L8,0", "M0,0 L8,8"],
        };
        let mut pattern = Element::new("pattern");
        pattern.set_attribute("id", id);
        pattern.set_attribute("width", "8");
        pattern.set_attribute("height", "8");
        pattern.set_attribute("patternUnits", "userSpaceOnUse");
        let mut rect = Element::new("rect");
        rect.set_attribute("width", "8");
        rect.set_attribute("height", "8");
        rect.set_attribute(
            "fill",
            &match background {
                Some(color) => rgb_hex(color),
                None => "none".to_string(),
            },
        );
        pattern.children.push(Node::Element(rect));
        for d in lines {
            let mut path = Element::new("path");
            path.set_attribute("d", d);
            path.set_attribute("stroke", &rgb_hex(foreground));
            path.set_attribute("stroke-width", "1");
            pattern.children.push(Node::Element(path));
        }
        let mut defs = Element::new("defs");
        defs.children.push(Node::Element(pattern));
        self.push_element(defs);
    }

    fn emit_shape(&mut self, kind: ShapeKind, paint: &PathPaint, _arrangement: Arrangement) {
        let mut element = Element::new(match kind {
            ShapeKind::Rect { .. } | ShapeKind::RoundedRect { .. } => "rect",
            ShapeKind::Ellipse { .. } => "ellipse",
        });
        match kind {
            ShapeKind::Rect {
                x,
                y,
                width,
                height,
            } => {
                element.set_attribute("x", &fmt4(x));
                element.set_attribute("y", &fmt4(y));
                element.set_attribute("width", &fmt4(width));
                element.set_attribute("height", &fmt4(height));
            }
            ShapeKind::RoundedRect {
                x,
                y,
                width,
                height,
                rx,
                ry,
            } => {
                element.set_attribute("x", &fmt4(x));
                element.set_attribute("y", &fmt4(y));
                element.set_attribute("width", &fmt4(width));
                element.set_attribute("height", &fmt4(height));
                element.set_attribute("rx", &fmt4(rx));
                element.set_attribute("ry", &fmt4(ry));
            }
            ShapeKind::Ellipse { cx, cy, rx, ry } => {
                element.set_attribute("cx", &fmt4(cx));
                element.set_attribute("cy", &fmt4(cy));
                element.set_attribute("rx", &fmt4(rx));
                element.set_attribute("ry", &fmt4(ry));
            }
        }
        // Shapes attach fill first, then stroke, then clip, then fallbacks.
        if let Some(fill) = &paint.fill {
            apply_raw_fill(&mut element, fill);
        }
        if let Some(stroke) = &paint.stroke {
            apply_raw_stroke(&mut element, stroke);
        }
        apply_clip(&mut element, paint.clip);
        if paint.fill.is_none() {
            element.set_attribute("fill", "none");
        }
        if paint.stroke.is_none() {
            element.set_attribute("stroke", "none");
        }
        self.push_element(element);
        self.push_text("\n");
    }

    fn emit_path(
        &mut self,
        geometry: &vector_model::PathGeometry,
        paint: &PathPaint,
        arrangement: Arrangement,
    ) {
        match arrangement {
            Arrangement::DeferredPath => {
                let mut element = Element::new("path");
                apply_clip_raw(&mut element, paint.clip);
                element.set_attribute("d", &path_d(geometry));
                // Paint lands at the closing action: fill first when both
                // paints are present.
                if let Some(fill) = &paint.fill {
                    apply_raw_fill(&mut element, fill);
                }
                if let Some(stroke) = &paint.stroke {
                    apply_raw_stroke(&mut element, stroke);
                }
                if paint.fill.is_none() {
                    element.set_attribute("fill", "none");
                }
                if paint.stroke.is_none() {
                    element.set_attribute("stroke", "none");
                }
                self.push_element(element);
                self.push_text("\n");
            }
            Arrangement::RegionStrokeFirst | Arrangement::FormClosed => {
                let mut element = Element::new("path");
                apply_clip_raw(&mut element, paint.clip);
                let mut d = path_d(geometry);
                d.push('Z');
                element.set_attribute("d", &d);
                if let Some(stroke) = &paint.stroke {
                    apply_raw_stroke(&mut element, stroke);
                }
                if let Some(fill) = &paint.fill {
                    apply_raw_fill(&mut element, fill);
                }
                if paint.fill.is_none() {
                    element.set_attribute("fill", "none");
                }
                if paint.stroke.is_none() {
                    element.set_attribute("stroke", "none");
                }
                self.push_element(element);
                self.push_text("\n");
            }
            Arrangement::RegionFillFirst => {
                let mut element = Element::new("path");
                apply_clip_raw(&mut element, paint.clip);
                element.set_attribute("d", &path_d(geometry));
                if let Some(fill) = &paint.fill {
                    apply_raw_fill(&mut element, fill);
                }
                if let Some(stroke) = &paint.stroke {
                    apply_raw_stroke(&mut element, stroke);
                }
                if paint.fill.is_none() {
                    element.set_attribute("fill", "none");
                }
                if paint.stroke.is_none() {
                    element.set_attribute("stroke", "none");
                }
                self.push_element(element);
                self.push_text("\n");
            }
            Arrangement::ContourOpen { leading_fill_space } => {
                let mut element = Element::new("path");
                apply_clip_raw(&mut element, paint.clip);
                element.set_attribute("d", &path_d(geometry));
                if let Some(stroke) = &paint.stroke {
                    apply_raw_stroke(&mut element, stroke);
                }
                let _ = leading_fill_space;
                element.set_attribute("fill", "none");
                if paint.stroke.is_none() {
                    element.set_attribute("stroke", "none");
                }
                self.push_element(element);
                self.push_text("\n");
            }
            Arrangement::AttrMap => {
                let mut element = Element::new("path");
                // Attribute-map order: fill, clip, stroke, width, extras, d last.
                match &paint.fill {
                    Some(vector_model::Fill {
                        style: vector_model::FillStyle::None,
                    }) => {
                        element.set_attribute("fill", "none");
                    }
                    Some(vector_model::Fill {
                        style: vector_model::FillStyle::HatchRef { id },
                    }) => {
                        element.set_attribute("fill", &format!("url(#{id})"));
                    }
                    Some(vector_model::Fill {
                        style: vector_model::FillStyle::Solid { color },
                    }) => {
                        element.set_attribute("fill", &rgb_hex(*color));
                    }
                    Some(vector_model::Fill {
                        style: vector_model::FillStyle::PatternRef { id },
                    }) => {
                        element.set_attribute("fill", &format!("url(#{id})"));
                    }
                    None => element.set_attribute("fill", "none"),
                }
                apply_clip(&mut element, paint.clip);
                match &paint.stroke {
                    Some(Stroke::NoneWithZeroWidth) => {
                        element.set_attribute("stroke", "none");
                        element.set_attribute("stroke-width", "0.0");
                    }
                    Some(Stroke::FromFill { color }) => {
                        element.set_attribute("stroke-width", "1px");
                        element.set_attribute("stroke", &rgb_hex(*color));
                    }
                    Some(stroke) => {
                        apply_attrmap_stroke(&mut element, stroke);
                    }
                    None => {}
                }
                element.set_attribute("d", &path_d(geometry));
                self.push_element(element);
                // No newline: the attribute-map family continues in-stream.
            }
            Arrangement::SolidRectFill => {
                let mut element = Element::new("path");
                let style = match &paint.fill {
                    Some(vector_model::Fill {
                        style: vector_model::FillStyle::Solid { color },
                    }) => {
                        format!("fill:{}", rgb_hex(*color))
                    }
                    _ => String::new(),
                };
                element.set_attribute("style", &style);
                element.set_attribute("d", &path_d(geometry));
                self.push_element(element);
                // No newline.
            }
            Arrangement::BareShape => unreachable!("BareShape uses the BareRect body"),
        }
    }

    fn emit_text(&mut self, text: &vector_model::TextDraw, paint: &PathPaint) {
        let mut element = Element::new("text");
        apply_clip(&mut element, paint.clip);
        if let Some(face) = &text.face {
            element.set_attribute("font-family", face);
        }
        element.set_attribute("fill", &rgb_hex(text.color));
        if let Some(rotation) = text.rotation {
            let raise = text.font_height * 0.9;
            let mut value = String::from("rotate(");
            value.push_str(&rotation.to_string());
            value.push_str(", ");
            append_fmt4(&mut value, text.x);
            value.push_str(", ");
            append_fmt4(&mut value, text.y + raise);
            value.push_str(") translate(0, ");
            append_fmt4(&mut value, raise);
            value.push(')');
            element.set_attribute("transform", &value);
        }
        if text.italic {
            element.set_attribute("font-style", "italic");
        }
        // The reference quirk `style ="..."` collapses to the clean attribute
        // under the SVG model's representability rules.
        element.set_attribute("style", "white-space:pre;");
        if text.underline && text.strikeout {
            element.set_attribute("text-decoration", "line-through,underline");
        } else if text.underline {
            element.set_attribute("text-decoration", "underline");
        } else if text.strikeout {
            element.set_attribute("text-decoration", "line-through");
        }
        if text.weight != 0 {
            element.set_attribute("font-weight", &text.weight.to_string());
        }
        element.set_attribute(
            "text-anchor",
            match text.anchor {
                TextAnchor::Start => "start",
                TextAnchor::Middle => "middle",
                TextAnchor::End => "end",
            },
        );
        let y = match text.baseline {
            BaselineMode::AtY => text.y,
            BaselineMode::Raised => text.y + (text.font_height * 0.9),
        };
        element.set_attribute("x", &fmt4(text.x));
        element.set_attribute("y", &fmt4(y));
        element.set_attribute("font-size", &fmt4(text.font_height));
        element.children.push(Node::CData(text.content.clone()));
        self.push_element(element);
        self.push_text("\n");
    }

    fn emit_image(&mut self, image: &vector_model::ImageDraw, paint: &PathPaint) {
        let href = image_href(&image.raster);
        match &image.arrangement {
            ImageArrangement::Calibrated {
                width,
                height,
                x,
                y,
            } => {
                let mut element = Element::new("image");
                element.set_attribute("width", &fmt4(*width));
                element.set_attribute("height", &fmt4(*height));
                element.set_attribute("x", &fmt4(*x));
                element.set_attribute("y", &fmt4(*y));
                apply_clip(&mut element, paint.clip);
                element.set_attribute("xlink:href", &href);
                self.push_element(element);
                self.push_text("\n");
            }
            ImageArrangement::Placed {
                x,
                y,
                width,
                height,
            } => {
                let mut element = Element::new("image");
                element.set_attribute("x", &ruby_float_to_s(*x));
                element.set_attribute("y", &ruby_float_to_s(*y));
                element.set_attribute("width", &ruby_float_to_s(*width));
                element.set_attribute("height", &ruby_float_to_s(*height));
                element.set_attribute("xlink:href", &href);
                element.set_attribute("preserveAspectRatio", "none");
                self.push_element(element);
                // No newline.
            }
            ImageArrangement::PlacedIntegral {
                x,
                y,
                width,
                height,
            } => {
                let mut element = Element::new("image");
                element.set_attribute("x", &ruby_int_to_s(*x as f64));
                element.set_attribute("y", &ruby_int_to_s(*y as f64));
                element.set_attribute("width", &ruby_int_to_s(*width as f64));
                element.set_attribute("height", &ruby_int_to_s(*height as f64));
                element.set_attribute("xlink:href", &href);
                element.set_attribute("preserveAspectRatio", "none");
                self.push_element(element);
                // No newline.
            }
        }
    }

    fn emit_bare_rect(&mut self, rect: &vector_model::BareRect) {
        let mut element = Element::new("rect");
        element.set_attribute("x", &rect.x);
        element.set_attribute("y", &rect.y);
        element.set_attribute("width", &rect.width);
        element.set_attribute("height", &rect.height);
        element.set_attribute("fill", &rect.fill);
        self.push_element(element);
        // No newline.
    }
}

/// The linear form of path geometry: `M x,y L x,y C ... Z ` with the
/// geometry's join style.
#[must_use]
pub fn path_d(geometry: &vector_model::PathGeometry) -> String {
    let mut d = String::new();
    let tuple = geometry.join == CoordJoin::Tuple;
    for segment in &geometry.segments {
        match segment {
            Segment::MoveTo { x, y } => {
                d.push_str("M ");
                append_point(&mut d, *x, *y, tuple);
            }
            Segment::LineTo { x, y } => {
                d.push_str("L ");
                append_point(&mut d, *x, *y, tuple);
            }
            Segment::CubicTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                d.push_str("C ");
                append_point(&mut d, *x1, *y1, tuple);
                append_point(&mut d, *x2, *y2, tuple);
                append_point(&mut d, *x, *y, tuple);
            }
            Segment::ArcTo {
                rx,
                ry,
                rotation: _,
                large,
                sweep,
                x,
                y,
            } => {
                d.push_str("A ");
                append_point(&mut d, *rx, *ry, tuple);
                d.push_str("0 ");
                d.push_str(if *large { "1" } else { "0" });
                d.push(' ');
                d.push_str(if *sweep { "1" } else { "0" });
                d.push(' ');
                append_point(&mut d, *x, *y, tuple);
            }
            Segment::Close => d.push_str("Z "),
        }
    }
    d
}

fn append_point(out: &mut String, x: f64, y: f64, tuple: bool) {
    if tuple {
        append_point_tuple(out, x, y);
    } else {
        append_point_sequence(out, x, y);
    }
}

/// The raw-stream fill attachment (fill_draw): the solid fill's leading
/// space is the reference fill-rule quirk.
fn apply_raw_fill(element: &mut Element, fill: &vector_model::Fill) {
    match &fill.style {
        vector_model::FillStyle::None => element.set_attribute("fill", "none"),
        vector_model::FillStyle::Solid { color } => {
            // The leading space is emitted by appending the fill attribute
            // after a spacing-only pseudo-entry; in the DOM model it lands
            // as a single separator space, which the quirk normalization
            // produces on the reference side too.
            element.set_attribute("fill", &rgb_hex(*color));
        }
        vector_model::FillStyle::PatternRef { id } => {
            element.set_attribute("fill", &format!("#{id}"));
        }
        vector_model::FillStyle::HatchRef { id } => {
            element.set_attribute("fill", &format!("url(#{id})"));
        }
    }
}

/// The raw-stream stroke attachment (stroke_draw).
fn apply_raw_stroke(element: &mut Element, stroke: &Stroke) {
    match stroke {
        Stroke::NoneWithZeroWidth => {
            element.set_attribute("stroke", "none");
            element.set_attribute("stroke-width", "0.0");
        }
        Stroke::FromFill { color } => {
            element.set_attribute("stroke-width", "1px");
            element.set_attribute("stroke", &rgb_hex(*color));
        }
        Stroke::Solid {
            color,
            width,
            dash,
            cap,
            join,
        } => {
            element.set_attribute("stroke", &rgb_hex(*color));
            apply_width(element, width);
            if let Some(dash) = dash {
                let mut value = String::new();
                for (i, entry) in dash.0.iter().enumerate() {
                    if i > 0 {
                        value.push(',');
                    }
                    append_fmt4(&mut value, *entry);
                }
                element.set_attribute("stroke-dasharray", &value);
            }
            if let Some(cap) = cap {
                element.set_attribute(
                    "stroke-linecap",
                    match cap {
                        LineCap::Round => "round",
                        LineCap::Square => "square",
                        LineCap::Butt => "butt",
                    },
                );
            }
            if let Some(join) = join {
                match join {
                    LineJoin::Round => element.set_attribute("stroke-linejoin", "round"),
                    LineJoin::Bevel => element.set_attribute("stroke-linejoin", "bevel"),
                    LineJoin::Miter { limit } => {
                        element.set_attribute("stroke-linejoin", "miter");
                        element.set_attribute("stroke-miterlimit", &fmt4(*limit));
                    }
                }
            }
        }
    }
}

/// The attribute-map stroke (stroke_attrs): plain values, no spacing quirks.
fn apply_attrmap_stroke(element: &mut Element, stroke: &Stroke) {
    let Stroke::Solid {
        color,
        width,
        dash,
        cap,
        join,
    } = stroke
    else {
        return;
    };
    element.set_attribute("stroke", &rgb_hex(*color));
    apply_width(element, width);
    if let Some(dash) = dash {
        let mut value = String::new();
        for (i, entry) in dash.0.iter().enumerate() {
            if i > 0 {
                value.push(',');
            }
            append_fmt4(&mut value, *entry);
        }
        element.set_attribute("stroke-dasharray", &value);
    }
    if let Some(cap) = cap {
        element.set_attribute(
            "stroke-linecap",
            match cap {
                LineCap::Round => "round",
                LineCap::Square => "square",
                LineCap::Butt => "butt",
            },
        );
    }
    if let Some(join) = join {
        match join {
            LineJoin::Round => element.set_attribute("stroke-linejoin", "round"),
            LineJoin::Bevel => element.set_attribute("stroke-linejoin", "bevel"),
            LineJoin::Miter { limit } => {
                element.set_attribute("stroke-linejoin", "miter");
                element.set_attribute("stroke-miterlimit", &fmt4(*limit));
            }
        }
    }
}

fn apply_width(element: &mut Element, width: &Width) {
    match width {
        Width::OnePixel => element.set_attribute("stroke-width", "1px"),
        Width::OneLegacy => element.set_attribute("stroke-width", "1.0000"),
        Width::Scaled(value) => element.set_attribute("stroke-width", &fmt4(*value)),
        Width::Count(value) => element.set_attribute("stroke-width", &value.to_string()),
        Width::Zero => element.set_attribute("stroke-width", "0.0"),
    }
}

fn apply_clip(element: &mut Element, clip: Option<u32>) {
    if let Some(id) = clip {
        element.set_attribute("clip-path", &format!("url(#clip-{id})"));
    }
}

/// The raw-stream clip attachment (clipset_draw): leading and trailing
/// space; in the DOM model a single attribute entry.
fn apply_clip_raw(element: &mut Element, clip: Option<u32>) {
    apply_clip(element, clip);
}

#[must_use]
fn rgb_hex(color: vector_model::Color) -> String {
    format!("#{:02X}{:02X}{:02X}", color.red, color.green, color.blue)
}

fn image_href(raster: &vector_model::Raster) -> String {
    match &raster.format {
        RasterFormat::Png => {
            format!("data:image/png;base64,{}", base64_encode(&raster.payload))
        }
        RasterFormat::Jpeg { zero_padded: true } => {
            format!("data:image/jpg;base64,{}", base64_encode(&raster.payload))
        }
        RasterFormat::Jpeg { zero_padded: false } => {
            format!("data:image/jpeg;base64,{}", base64_encode(&raster.payload))
        }
    }
}
