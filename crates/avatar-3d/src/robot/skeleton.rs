//! The skeleton's trunk bones, the anchors, and the bones' rest matrices.

use avatar_small_robots::layout::Frame;

use super::build::Builder;
use super::solids::V3;
use super::{Anchor, Bone, UNIT};

/// The extra height of `headTop` above the highest rest point, in model
/// units (the motion conventions' value).
pub const HEAD_TOP_CLEARANCE: f64 = 0.06;

/// Adds `root` at the contact point, `body` at the base of the lowest hull,
/// `neck` at the neck's base when the robot has one, and `head` at the
/// head's base, its pivot in the 2D drawing. A pod's head pivot is its
/// body's: the 2D type turns a pod's whole hull as one.
pub(super) fn trunk(b: &mut Builder) {
    let root = b.bone("root", None, [0.0; 3]);
    let body = b.bone("body", Some(root), b.at(Frame::UpperBody, [0.0, 0.0], 0.0));
    let mut parent = body;
    if let Some(neck) = b.layout.joints.neck {
        parent = b.bone("neck", Some(body), b.at(Frame::UpperBody, neck.base, 0.0));
    }
    let head = b.at(Frame::Head, [0.0, 0.0], 0.0);
    b.bone("head", Some(parent), head);
}

/// The anchors: `ground` at the contact point; `headTop` above the highest
/// point of the rest pose plus [`HEAD_TOP_CLEARANCE`]; `face` at the face
/// region's centre on the head's front; and `hand.L`, `hand.R` at each hand's
/// centre.
pub(super) fn anchors(b: &Builder) -> Vec<Anchor> {
    let head = b.find("head");
    let head_joint = scale(b.bones[head].2);
    let top = b
        .parts
        .iter()
        .flat_map(|p| p.mesh.positions.iter())
        .map(|p| p[1])
        .fold(f64::NEG_INFINITY, f64::max);
    let mut anchors = vec![
        Anchor {
            name: "ground",
            bone: None,
            offset: [0.0; 3],
        },
        Anchor {
            name: "headTop",
            bone: Some(head),
            offset: [
                0.0 - head_joint[0],
                top + HEAD_TOP_CLEARANCE - head_joint[1],
                0.0 - head_joint[2],
            ]
            .map(|v| v + 0.0),
        },
    ];
    let face = super::hulls::face_centre(b);
    anchors.push(Anchor {
        name: "face",
        bone: Some(head),
        offset: sub(face, head_joint),
    });
    for (side, name) in [(0, "hand.L"), (1, "hand.R")] {
        let Some(arm) = &b.layout.arms[side] else {
            continue;
        };
        let wrist = b.find(["wrist.L", "wrist.R"][side]);
        let centre = super::limbs::hand_centre(b, arm);
        anchors.push(Anchor {
            name,
            bone: Some(wrist),
            offset: sub(scale(centre), scale(b.bones[wrist].2)),
        });
    }
    anchors
}

/// The finished bones, in model units, with their rest matrices.
pub(super) fn finish(bones: &[(String, Option<usize>, V3)]) -> Vec<Bone> {
    bones
        .iter()
        .map(|(name, parent, joint)| {
            let j = scale(*joint);
            Bone {
                name: name.clone(),
                parent: *parent,
                joint: j,
                rest: [
                    1.0, 0.0, 0.0, 0.0, //
                    0.0, 1.0, 0.0, 0.0, //
                    0.0, 0.0, 1.0, 0.0, //
                    j[0], j[1], j[2], 1.0,
                ],
            }
        })
        .collect()
}

fn scale(p: V3) -> V3 {
    p.map(|v| v * UNIT + 0.0)
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]].map(|v| v + 0.0)
}
