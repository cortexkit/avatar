//! A whole head: every part decoded from the genome, posed in each mood and
//! placed in the 240-unit frame.

use avatar_svg::wild::GENOME_LEN;
use avatar_svg::Mood;

use crate::beard::{beard_kind_of, make_beard, Beard, BeardInputs};
use crate::face::{make_brows, make_eyes, make_mouth, make_nose, Brows, Eyes, Mouth, Nose};
use crate::genes::Genes;
use crate::geom::{box_of, clamp, join_box, matrix_of, min, pad_box, through, BoxF, Pt};
use crate::hair::{make_hair, Hair, HairInputs};
use crate::head::{layout_face, make_ears, make_head, make_neck, Ears, FaceLayout, Head, Neck};
use crate::mood::{head_ops, make_marks, make_temper, pose_of, Marks, Pose, Temper, MOODS};
use crate::palette::{build_palette, Palette};

/// Where a head stands in the frame: moved by (`x`, `y`) and scaled by `s`.
#[derive(Clone, Copy, Debug)]
pub struct Fit {
    pub s: f64,
    pub x: f64,
    pub y: f64,
    /// Whether the width, not the height, limits the scale.
    pub wide: bool,
}

/// Everything one genome decodes to.
#[derive(Clone, Debug)]
pub struct Creature {
    pub palette: Palette,
    pub head: Head,
    pub face: FaceLayout,
    pub eyes: Eyes,
    pub brows: Brows,
    pub nose: Nose,
    pub mouth: Mouth,
    pub ears: Ears,
    pub neck: Option<Neck>,
    pub hair: Hair,
    pub beard: Beard,
    pub marks: Marks,
    pub temper: Temper,
    /// How far the head's tip in each mood is cut back to fit the frame.
    pub trim: f64,
    /// Everything that moves with the head, as points, for measuring.
    pub reach: Vec<Pt>,
    /// The head posed in each mood, in [`MOODS`] order.
    pub poses: Vec<Pose>,
    pub fit: Fit,
}

impl Creature {
    pub fn pose(&self, mood: Mood) -> &Pose {
        &self.poses[MOODS
            .iter()
            .position(|&m| m == mood)
            .expect("every mood is posed")]
    }
}

/// Decodes a stored genome into the head it draws.
pub fn decode(bytes: &[u8; GENOME_LEN]) -> Creature {
    let g = Genes::new(bytes);
    let palette = build_palette(&g);
    let head = make_head(&g);
    let face = layout_face(&g, &head);
    let eyes = make_eyes(&g, &head, &face);
    let brows = make_brows(&g, &eyes, head.size);
    let nose = make_nose(&g, head.size);
    let mouth = make_mouth(&g, &head, &face);
    let ears = make_ears(&g, &head, &face);
    let style = palette.hair_style;
    let neck = make_neck(&g, &head, style);
    // Facial hair is chosen before the hair is built: sideburns stop short above a beard.
    let beard_kind = beard_kind_of(&g, style);
    let hair = make_hair(
        &g,
        &HairInputs {
            head: &head,
            face: &face,
            eyes: &eyes,
            brows: &brows,
            ears: &ears,
            neck: neck.as_ref(),
            beard_on_jaw: beard_kind.on_jaw(),
        },
        style,
    );
    let beard = make_beard(
        &g,
        &BeardInputs {
            head: &head,
            face: &face,
            mouth: &mouth,
            ears: &ears,
            palette: &palette,
        },
        beard_kind,
    );
    let marks = make_marks(&g, &brows, &eyes, hair.fore, &beard, &mouth, &head);
    let temper = make_temper(&g);
    let mut reach = head.poly.clone();
    reach.extend(ears.shape(&head, -1.0).pts);
    reach.extend(ears.shape(&head, 1.0).pts);
    hair.front.reach_pts(&mut reach);
    hair.back.reach_pts(&mut reach);
    beard.mass.reach_pts(&mut reach);
    let mut c = Creature {
        palette,
        head,
        face,
        eyes,
        brows,
        nose,
        mouth,
        ears,
        neck,
        hair,
        beard,
        marks,
        temper,
        trim: 1.0,
        reach,
        poses: Vec::new(),
        fit: Fit {
            s: 1.0,
            x: 0.0,
            y: 0.0,
            wide: false,
        },
    };
    // A head too wide for the frame when it tips is tipped less, down to
    // about half as far, before it is drawn smaller.
    let mut pass = 0;
    loop {
        c.poses = MOODS
            .iter()
            .map(|&m| pose_of(&c.temper, &c.eyes, &c.mouth, c.trim, m))
            .collect();
        c.fit = fit(&c);
        if c.fit.s >= 0.999 || !c.fit.wide || c.trim <= 0.5 || pass >= 6 {
            break;
        }
        c.trim *= 0.86;
        pass += 1;
    }
    c
}

/// The box one mood's drawing fills, before the fit.
fn mood_box(c: &Creature, p: &Pose) -> BoxF {
    let mut b = pad_box(
        box_of(&through(&matrix_of(&head_ops(&c.head, p)), &c.reach)),
        3.0,
    );
    if let Some(n) = &c.neck {
        b = join_box(Some(b), pad_box(box_of(&n.pts), 1.5));
    }
    b
}

/// The head is drawn at its own size, and smaller only if some mood would
/// otherwise leave the frame. The head at rest is centred in the frame.
fn fit(c: &Creature) -> Fit {
    let mut all = None;
    let mut rest = None;
    for p in &c.poses {
        let b = mood_box(c, p);
        if p.mood == Mood::Neutral {
            rest = Some(b);
        }
        all = Some(join_box(all, b));
    }
    let all = all.expect("seven moods");
    let rest = rest.expect("a neutral mood");
    let w = all[2] - all[0];
    let h = all[3] - all[1];
    let s = min(min(1.0, 224.0 / w), 226.0 / h);
    let x = clamp(
        120.0 - s * (rest[0] + rest[2]) / 2.0,
        8.0 - s * all[0],
        232.0 - s * all[2],
    );
    let y = clamp(
        120.0 - s * (rest[1] + rest[3]) / 2.0,
        7.0 - s * all[1],
        233.0 - s * all[3],
    );
    Fit {
        s,
        x,
        y,
        wide: 224.0 / w < 226.0 / h,
    }
}
