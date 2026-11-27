//! Reading the genes: which body plan, parts and face a genome asks for.
//!
//! All the genes this type adds are read from byte 1024 on, bit by bit;
//! bytes 256 to 1023 are never read. The body colour is read from bytes 160
//! to 162 exactly as creature.wild reads it. A gene is read only when the
//! creature expresses it, so the order of reads below is part of the type.

use crate::grid::Corner;

/// Where each group of genes starts (byte offsets).
const DNA_BODY: usize = 1024;
const DNA_TOP: usize = 1040;
const DNA_ARMS: usize = 1048;
const DNA_LEGS: usize = 1052;
const DNA_TAIL: usize = 1056;
const DNA_COAT: usize = 1066;
const DNA_PATTERN: usize = 1076;
const DNA_FACE: usize = 1104;
pub(crate) const DNA_COLOUR: usize = 1120;
const DNA_TEMPER: usize = 1128;

/// Reads genes as bit fields, lowest bit first.
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Reader { bytes, pos: 0 }
    }

    /// Moves to a byte offset.
    pub(crate) fn at(&mut self, byte_offset: usize) {
        self.pos = byte_offset * 8;
    }

    /// The next `n` bits as a number, lowest bit first. Bits past the end
    /// of the genome read as zero.
    pub(crate) fn bits(&mut self, n: u32) -> u32 {
        let mut v = 0;
        for i in 0..n {
            v |= self.bit_at(self.pos) << i;
            self.pos += 1;
        }
        v
    }

    pub(crate) fn bit(&mut self) -> u32 {
        self.bits(1)
    }

    /// The bit at an absolute bit index, without moving.
    pub(crate) fn bit_at(&self, p: usize) -> u32 {
        let byte = self.bytes.get(p >> 3).copied().unwrap_or(0);
        u32::from((byte >> (p & 7)) & 1)
    }

    /// One entry of a table whose length is a power of two, chosen by as
    /// many bits as it takes to address it.
    pub(crate) fn pick<T: Copy>(&mut self, table: &[T]) -> T {
        let mut n = 0;
        while (1usize << n) < table.len() {
            n += 1;
        }
        table[self.bits(n) as usize % table.len()]
    }
}

/// Defines a gene's choices with the reference's name for each.
macro_rules! gene_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident = $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            /// Every choice, in the order the gene's bits select them.
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// The reference's name for the choice.
            pub fn name(self) -> &'static str {
                match self {
                    $($name::$variant => $text),+
                }
            }
        }
    };
}

gene_enum!(
    /// The body plan: one lobe, a mound, a head on a small torso, or a head
    /// welded to a wider belly.
    Plan { Blob = "blob", Mound = "mound", Chibi = "chibi", Pear = "pear" }
);
gene_enum!(
    /// What grows on top of the head.
    TopKind {
        None = "none", Cat = "cat", Round = "round", Mouse = "mouse", Long = "long",
        Goblin = "goblin", Bat = "bat", Lynx = "lynx", Nub = "nub", Horns = "horns",
        Bull = "bull", Knobs = "knobs", Antlers = "antlers", Antennae = "antennae",
        Unicorn = "unicorn", Frills = "frills",
    }
);
gene_enum!(
    /// How an ear is finished.
    EarDetail { Plain = "plain", Inner = "inner", Dark = "dark", Knobs = "knobs" }
);
gene_enum!(
    /// What grows at the sides.
    SideKind {
        Nub = "nub", Short = "short", Long = "long", Claw = "claw", Flipper = "flipper",
        Tentacle = "tentacle", Wings = "wings", None = "none",
    }
);
gene_enum!(
    /// The kind of wing.
    WingKind { Bat = "bat", Feather = "feather", Insect = "insect", Tiny = "tiny" }
);
gene_enum!(
    /// What the body stands on.
    LegKind {
        Feet = "feet", Legs = "legs", Bird = "bird", Quad = "quad", Tentacles = "tentacles",
        Wisp = "wisp", Crab = "crab", None = "none",
    }
);
gene_enum!(
    /// The tail.
    TailKind {
        None = "none", Stub = "stub", Curl = "curl", Bushy = "bushy", Devil = "devil",
        Lizard = "lizard", Fluke = "fluke", Rat = "rat",
    }
);
gene_enum!(
    /// How the body is shaded and what covers its edge.
    CoatKind {
        Matte = "matte", Flat = "flat", Glossy = "glossy", Sleek = "sleek", Furry = "furry",
        Spiky = "spiky", Woolly = "woolly", Warty = "warty",
    }
);
gene_enum!(
    /// The pale patch on the lower body.
    Belly { None = "none", Round = "round", Low = "low", Small = "small" }
);
gene_enum!(
    /// Markings on the coat.
    Marks {
        None = "none", Spots = "spots", PaleSpots = "pale spots", Patches = "patches",
        OnePatch = "one patch", Points = "points", SpotsAndPoints = "spots and points",
        PatchesAndPoints = "patches and points",
    }
);
gene_enum!(
    /// How the eyes are set.
    EyePlan {
        Pair = "pair", Wide = "wide", Close = "close", Big = "big", Cyclops = "cyclops",
        Three = "three", Frog = "frog", Stalks = "stalks",
    }
);
gene_enum!(
    /// How an eye is drawn.
    EyeStyle {
        Bead = "bead", Iris = "iris", Sparkle = "sparkle", Tall = "tall", Sclera = "sclera",
        Dot = "dot", Slit = "slit", Lazy = "lazy",
    }
);
gene_enum!(
    /// The resting eyelid.
    LidGene { Open = "open", Lidded = "lidded", Heavy = "heavy", Stern = "stern" }
);
gene_enum!(
    /// The mouth.
    MouthKind {
        Smile = "smile", Flat = "flat", Cat = "cat", Fangs = "fangs", Buck = "buck",
        Beak = "beak", Maw = "maw", Tiny = "tiny",
    }
);
gene_enum!(
    /// What lies round the mouth.
    Muzzle { None = "none", Nose = "nose", Muzzle = "muzzle", MuzzleAndNose = "muzzle and nose" }
);
gene_enum!(
    /// The cheeks.
    Cheeks { None = "none", Blush = "blush", BigBlush = "big blush", Whiskers = "whiskers" }
);
gene_enum!(
    /// The resting brows.
    BrowGene { None = "none", Thin = "thin", Thick = "thick", Short = "short" }
);
gene_enum!(
    /// A one-sided quirk of the face.
    Quirk { None = "none", Smirk = "smirk", Snaggle = "snaggle", Blep = "blep" }
);
gene_enum!(
    /// How the face shows it is blocked.
    BlockedFace { Glare = "glare", Squeeze = "squeeze", Teeth = "teeth", Pout = "pout" }
);

/// The body's genes.
#[derive(Clone, Debug)]
pub struct BodyGenes {
    pub plan: Plan,
    /// The body's area in cells, before the fit.
    pub area: i32,
    pub aspect: f64,
    pub top: Corner,
    pub bot: Corner,
    /// How far down the widest row lies, 0..1.
    pub peak: f64,
    /// A mound's share of rows cut off flat at its base.
    pub cut_share: f64,
    /// A two-lobed body's share of its area in the head.
    pub head_share: f64,
    pub torso_ix: i32,
    pub torso_ratio: f64,
    /// Rows of the second lobe that show; 0 for a body of one lobe.
    pub torso_rows: i32,
    pub torso_shape: Corner,
    pub overlap: i32,
}

/// What grows on top of the head.
#[derive(Clone, Debug)]
pub struct TopGenes {
    pub kind: TopKind,
    pub size: i32,
    pub wide: i32,
    pub place: i32,
    pub lean: i32,
    pub quirk: i32,
    /// Read only for ears.
    pub detail: Option<EarDetail>,
}

/// Arms or wings.
#[derive(Clone, Debug)]
pub struct SideGenes {
    pub kind: SideKind,
    /// Read only for wings.
    pub wing: Option<WingKind>,
    pub span: i32,
    pub high: i32,
    pub deep: i32,
    pub len: i32,
    pub thick: i32,
}

/// Legs.
#[derive(Clone, Debug)]
pub struct LegGenes {
    pub kind: LegKind,
    pub len: i32,
    pub spread: i32,
    pub big: i32,
    pub side: i32,
}

/// The tail.
#[derive(Clone, Debug)]
pub struct TailGenes {
    pub kind: TailKind,
    pub side: i32,
    pub len: i32,
    pub curl: i32,
    pub tipped: bool,
}

/// The coat.
#[derive(Clone, Debug)]
pub struct CoatGenes {
    pub kind: CoatKind,
    pub grain: i32,
    /// The byte from which one bit per spine, tuft or wart is read.
    pub at: usize,
}

/// Belly and markings.
#[derive(Clone, Debug)]
pub struct PatternGenes {
    pub belly: Belly,
    pub belly_size: i32,
    pub marks: Marks,
    pub count: i32,
    pub big: i32,
    pub mirror: bool,
    /// The byte from which where each marking sits is read.
    pub at: usize,
}

/// The face.
#[derive(Clone, Debug)]
pub struct FaceGenes {
    pub plan: EyePlan,
    pub style: EyeStyle,
    pub wide: i32,
    pub tall: i32,
    pub row: i32,
    pub lid: LidGene,
    pub mouth: MouthKind,
    pub mouth_wide: i32,
    pub mouth_drop: i32,
    pub muzzle: Muzzle,
    pub nose_dark: bool,
    pub cheeks: Cheeks,
    pub brows: BrowGene,
    pub quirk: Quirk,
    pub quirk_side: i32,
}

/// How the creature carries its moods.
#[derive(Clone, Debug)]
pub struct Temper {
    /// Which way it looks and leans: 1 or -1.
    pub side: i32,
    /// A smile closes its eyes.
    pub smile_shut: bool,
    /// Its tongue shows when it works.
    pub tongue: bool,
    pub blocked: BlockedFace,
    /// The side a tear runs from: 1 or -1.
    pub tear_side: i32,
    /// Asleep with its mouth open.
    pub yawn: bool,
    /// Happy lifts it two rows off the ground rather than one.
    pub hop: bool,
    /// Blocked crosses its arms.
    pub crossed: bool,
}

/// Every gene a creature reads.
#[derive(Clone, Debug)]
pub struct Genes {
    pub body: BodyGenes,
    pub face: FaceGenes,
    pub top: TopGenes,
    pub side: SideGenes,
    pub legs: LegGenes,
    pub tail: TailGenes,
    pub coat: CoatGenes,
    pub pattern: PatternGenes,
    pub temper: Temper,
}

fn int(v: u32) -> i32 {
    v as i32
}

pub(crate) fn read_genes(r: &mut Reader) -> Genes {
    // ---- body ----
    r.at(DNA_BODY);
    let plan = r.pick(Plan::ALL);
    let area = r.pick(&[120, 140, 160, 180, 200, 225, 255, 290]);
    let aspect_ix = r.bits(3) as usize;
    let mut aspect = [0.62, 0.72, 0.82, 0.92, 1.0, 1.1, 1.22, 1.38][aspect_ix];
    if plan == Plan::Mound {
        aspect = [0.74, 0.8, 0.86, 0.92, 1.0, 1.1, 1.22, 1.38][aspect_ix];
    }
    let mut top = r.pick(Corner::ALL);
    let bot = r.pick(Corner::ALL);
    let peak = r.pick(&[0.4, 0.48, 0.56, 0.64]);
    // A one-lobed body square above and below is a box, and a mound with a
    // square top is a loaf: there the top is rounded off a little.
    if top == Corner::Square
        && (plan == Plan::Mound
            || (plan == Plan::Blob && (bot == Corner::Square || bot == Corner::Boxy)))
    {
        top = Corner::Boxy;
    }
    let mut body = BodyGenes {
        plan,
        area,
        aspect,
        top,
        bot,
        peak,
        cut_share: 0.0,
        head_share: 1.0,
        torso_ix: 0,
        torso_ratio: 0.0,
        torso_rows: 0,
        torso_shape: Corner::Round,
        overlap: 0,
    };
    if plan == Plan::Mound {
        body.cut_share = r.pick(&[0.16, 0.24, 0.32, 0.4]);
    } else if plan == Plan::Chibi || plan == Plan::Pear {
        let pear = plan == Plan::Pear;
        // A head above a second lobe is wider than tall and keeps most of the
        // body's size: it carries the face.
        body.aspect = [0.6, 0.66, 0.72, 0.78, 0.84, 0.9, 0.96, 1.04][aspect_ix];
        body.head_share = r.pick(if pear {
            &[0.6, 0.68, 0.76, 0.84]
        } else {
            &[0.7, 0.78, 0.86, 0.94]
        });
        body.torso_ix = int(r.bits(2));
        body.torso_ratio = [0.5, 0.62, 0.74, 0.86][body.torso_ix as usize];
        body.torso_rows = r.pick(if pear { &[4, 5, 6, 7] } else { &[3, 4, 5, 6] });
        body.torso_shape = r.pick(Corner::ALL);
        body.overlap = 2 + int(r.bit());
    }

    // The eye plan is read first: eyes carried above the head leave no room
    // for ears.
    r.at(DNA_FACE);
    let eye_plan = r.pick(EyePlan::ALL);
    let mounted = eye_plan == EyePlan::Frog || eye_plan == EyePlan::Stalks;

    // ---- what grows on top ----
    let mut top_genes = TopGenes {
        kind: TopKind::None,
        size: 0,
        wide: 0,
        place: 0,
        lean: 0,
        quirk: 0,
        detail: None,
    };
    if !mounted {
        r.at(DNA_TOP);
        let mut kind = r.pick(TopKind::ALL);
        // A mound sits flat with no legs to show; bare, or with knobs too
        // small to see, it reads as a loaf, so it always grows something
        // that can be seen, and nothing that grows from its sides.
        if plan == Plan::Mound {
            kind = match kind {
                TopKind::None => TopKind::Cat,
                TopKind::Nub => TopKind::Round,
                TopKind::Knobs => TopKind::Long,
                TopKind::Goblin => TopKind::Bat,
                TopKind::Frills => TopKind::Lynx,
                other => other,
            };
        }
        top_genes.kind = kind;
    }
    if top_genes.kind != TopKind::None {
        top_genes.size = int(r.bits(2));
        top_genes.wide = int(r.bit());
        top_genes.place = int(r.bits(2));
        top_genes.lean = int(r.bit());
        top_genes.quirk = int(r.bits(2));
        if is_ear(top_genes.kind) {
            top_genes.detail = Some(r.pick(EarDetail::ALL));
        }
    }

    // ---- arms or wings ----
    r.at(DNA_ARMS);
    let mut side = SideGenes {
        kind: r.pick(SideKind::ALL),
        wing: None,
        span: 0,
        high: 0,
        deep: 0,
        len: 0,
        thick: 0,
    };
    if side.kind == SideKind::Wings {
        side.wing = Some(r.pick(WingKind::ALL));
        side.span = int(r.bits(2));
        side.high = int(r.bit());
        side.deep = int(r.bit());
    } else if side.kind != SideKind::None {
        side.high = int(r.bits(2));
        side.len = int(r.bits(2));
        side.thick = int(r.bit());
    }

    // ---- legs ----
    let mut legs = LegGenes {
        kind: LegKind::None,
        len: 0,
        spread: 0,
        big: 0,
        side: 0,
    };
    if plan != Plan::Mound {
        r.at(DNA_LEGS);
        legs.kind = r.pick(LegKind::ALL);
        if legs.kind != LegKind::None {
            legs.len = int(r.bits(2));
            legs.spread = int(r.bits(2));
            legs.big = int(r.bit());
            if legs.kind == LegKind::Wisp {
                legs.side = int(r.bit());
            }
        }
    }

    // ---- tail ----
    let mut tail = TailGenes {
        kind: TailKind::None,
        side: 0,
        len: 0,
        curl: 0,
        tipped: false,
    };
    if legs.kind != LegKind::Wisp && legs.kind != LegKind::Tentacles {
        r.at(DNA_TAIL);
        tail.kind = r.pick(TailKind::ALL);
        if tail.kind != TailKind::None {
            tail.side = int(r.bit());
            tail.len = int(r.bits(2));
            tail.curl = int(r.bits(2));
            if tail.kind == TailKind::Bushy {
                // A pale tip, as on a fox.
                tail.tipped = r.bit() != 0;
            }
        }
    }

    // ---- coat ----
    r.at(DNA_COAT);
    let coat_kind = r.pick(CoatKind::ALL);
    let mut coat = CoatGenes {
        kind: coat_kind,
        grain: 0,
        at: DNA_COAT + 1,
    };
    if matches!(
        coat_kind,
        CoatKind::Furry | CoatKind::Spiky | CoatKind::Woolly | CoatKind::Warty
    ) {
        coat.grain = int(r.bits(2));
    }

    // ---- belly and markings ----
    r.at(DNA_PATTERN);
    let belly = r.pick(Belly::ALL);
    let belly_size = if belly != Belly::None {
        int(r.bits(2))
    } else {
        0
    };
    r.at(DNA_PATTERN + 1);
    let marks = r.pick(Marks::ALL);
    let mut pattern = PatternGenes {
        belly,
        belly_size,
        marks,
        count: 0,
        big: 0,
        mirror: false,
        at: DNA_PATTERN + 2,
    };
    // Points alone have nothing to count or place; only spots can come in
    // mirrored pairs.
    if marks != Marks::None && marks != Marks::Points {
        pattern.count = int(r.bits(2));
        pattern.big = int(r.bit());
        if !matches!(
            marks,
            Marks::Patches | Marks::OnePatch | Marks::PatchesAndPoints
        ) {
            pattern.mirror = r.bit() != 0;
        }
    }

    // ---- face ----
    r.at(DNA_FACE);
    r.pos += 3; // the eye plan, read above
    let style = r.pick(EyeStyle::ALL);
    let wide = int(r.bit());
    let tall = int(r.bits(2));
    let row = int(r.bits(2));
    let lid = r.pick(LidGene::ALL);
    r.at(DNA_FACE + 2);
    let mut mouth = r.pick(MouthKind::ALL);
    if plan == Plan::Mound && mouth == MouthKind::Tiny {
        mouth = MouthKind::Smile; // a mound's mouth must be seen
    }
    let mouth_wide = int(r.bits(2));
    let mouth_drop = int(r.bit());
    let mut muzzle = Muzzle::None;
    let mut nose_dark = false;
    if mouth != MouthKind::Beak {
        muzzle = r.pick(Muzzle::ALL);
        if muzzle == Muzzle::Nose || muzzle == Muzzle::MuzzleAndNose {
            nose_dark = r.bit() != 0;
        }
    }
    r.at(DNA_FACE + 4);
    let cheeks = r.pick(Cheeks::ALL);
    let brows = r.pick(BrowGene::ALL);
    let quirk = r.pick(Quirk::ALL);
    // Which way a smirk pulls, or which fang shows.
    let quirk_side = if quirk == Quirk::Smirk || quirk == Quirk::Snaggle {
        int(r.bit())
    } else {
        0
    };
    let face = FaceGenes {
        plan: eye_plan,
        style,
        wide,
        tall,
        row,
        lid,
        mouth,
        mouth_wide,
        mouth_drop,
        muzzle,
        nose_dark,
        cheeks,
        brows,
        quirk,
        quirk_side,
    };

    // ---- temperament: seen in the moods ----
    r.at(DNA_TEMPER);
    let temper = Temper {
        side: if r.bit() != 0 { 1 } else { -1 },
        smile_shut: r.bit() != 0,
        tongue: r.bit() != 0,
        blocked: r.pick(BlockedFace::ALL),
        tear_side: if r.bit() != 0 { 1 } else { -1 },
        yawn: r.bit() != 0,
        hop: r.bit() != 0,
        crossed: r.bit() != 0,
    };

    Genes {
        body,
        face,
        top: top_genes,
        side,
        legs,
        tail,
        coat,
        pattern,
        temper,
    }
}

/// Whether what grows on top is an ear.
pub(crate) fn is_ear(kind: TopKind) -> bool {
    matches!(
        kind,
        TopKind::Cat
            | TopKind::Round
            | TopKind::Mouse
            | TopKind::Long
            | TopKind::Goblin
            | TopKind::Bat
            | TopKind::Lynx
            | TopKind::Nub
    )
}
