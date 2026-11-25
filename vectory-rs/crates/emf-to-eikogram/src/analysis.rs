//! The two-pass deferred-path analysis (the reference renderer's
//! build_path_analysis): per begin-path action, and the world transforms
//! captured inside begin/end-path blocks split by whether drawing has
//! happened yet.

use std::collections::HashMap;

use emf_core::{RecordData, RecordSpan};

/// What closes a begin-path block (the first fill/stroke action after it,
/// resolved by one backward pass; another begin first, or none at all,
/// leaves End).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathAction {
    End,
    Fill,
    Stroke,
    StrokeFill,
    Abort,
}

/// A world-transform capture inside a path block: the raw matrix and the
/// modify mode (0 = assign, 1 identity, 2 left-multiply, 3 right-multiply).
#[derive(Clone, Debug)]
pub struct PathTransform {
    pub matrix: emf_core::XForm,
    pub mode: u32,
}

/// Both captured transform slots of one path block. The reference names them
/// wtBefore/wtAfter but assigns transforms recorded AFTER drawing to the
/// wt_before slot (applied at BeginPath) and transforms recorded BEFORE
/// drawing to wt_after (applied at EndPath); the naming is carried as-is.
#[derive(Clone, Debug, Default)]
pub struct CapturedTransforms {
    pub wt_before: Option<PathTransform>,
    pub wt_after: Option<PathTransform>,
}

/// Records that set pathDrawn=true in the reference analysis pre-pass.
/// SetWindowExtEx is deliberately on this list (a reference quirk).
fn is_drawing_record(record_type: u32) -> bool {
    matches!(
        record_type,
        2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 54 | 55 | 56 | 85 | 86 | 87 | 88 | 89 | 90 | 91 | 92
    )
}

/// The path-analysis kind per record type (None when irrelevant).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    BeginPath,
    EndPath,
    AbortPath,
    FillPath,
    StrokePath,
    StrokeAndFillPath,
    SetWorldTransform,
    ModifyWorldTransform,
    Drawing,
}

fn kind_of(record_type: u32) -> Option<Kind> {
    Some(match record_type {
        59 => Kind::BeginPath,
        60 => Kind::EndPath,
        68 => Kind::AbortPath,
        62 => Kind::FillPath,
        64 => Kind::StrokePath,
        63 => Kind::StrokeAndFillPath,
        35 => Kind::SetWorldTransform,
        36 => Kind::ModifyWorldTransform,
        t if is_drawing_record(t) => Kind::Drawing,
        _ => return None,
    })
}

/// The analysis result keyed by the begin-path record's index.
pub struct PathAnalysis {
    pub actions: HashMap<usize, PathAction>,
    pub transforms: HashMap<usize, CapturedTransforms>,
}

/// Pre-scans the records to build the action map and the transform captures.
#[must_use]
pub fn analyze(records: &[RecordSpan]) -> PathAnalysis {
    let kinds: Vec<Option<Kind>> = records
        .iter()
        .map(|span| kind_of(span.record_type()))
        .collect();

    // Backward pass: each BeginPath takes the first following action.
    let mut actions: HashMap<usize, PathAction> = HashMap::new();
    let mut following = PathAction::End;
    for i in (0..kinds.len()).rev() {
        match kinds[i] {
            Some(Kind::BeginPath) => {
                actions.insert(i, following);
                following = PathAction::End;
            }
            Some(Kind::FillPath) => following = PathAction::Fill,
            Some(Kind::StrokePath) => following = PathAction::Stroke,
            Some(Kind::StrokeAndFillPath) => following = PathAction::StrokeFill,
            Some(Kind::AbortPath) => following = PathAction::Abort,
            _ => {}
        }
    }

    // Forward pass: capture in-block transforms split at the first drawing.
    let mut transforms: HashMap<usize, CapturedTransforms> = HashMap::new();
    let mut in_path = false;
    let mut path_drawn = false;
    let mut cur_begin: Option<usize> = None;
    let mut captured = CapturedTransforms::default();
    for (i, span) in records.iter().enumerate() {
        match kinds[i] {
            Some(Kind::BeginPath) => {
                cur_begin = Some(i);
                captured = CapturedTransforms::default();
                in_path = true;
                path_drawn = false;
            }
            Some(Kind::EndPath) | Some(Kind::AbortPath) => {
                if let Some(begin) = cur_begin {
                    transforms.insert(begin, captured.clone());
                }
                in_path = false;
                path_drawn = false;
                cur_begin = None;
            }
            Some(Kind::SetWorldTransform) | Some(Kind::ModifyWorldTransform) => {
                if !in_path {
                    continue;
                }
                let (matrix, mode) = match span.data() {
                    RecordData::ModifyWorldTransform { xform, modify_mode } => {
                        (*xform, *modify_mode)
                    }
                    RecordData::SetWorldTransform { xform } => (*xform, 0),
                    _ => continue,
                };
                let pt = PathTransform { matrix, mode };
                // The reference's slot choice: post-drawing captures land in
                // wt_before, pre-drawing captures in wt_after.
                if path_drawn {
                    captured.wt_before = Some(pt);
                } else {
                    captured.wt_after = Some(pt);
                }
            }
            Some(Kind::Drawing) if in_path => {
                path_drawn = true;
            }
            _ => {}
        }
    }
    PathAnalysis {
        actions,
        transforms,
    }
}
