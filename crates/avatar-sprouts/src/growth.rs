//! Growth: an L-system whose rules the genome writes, the places on the body
//! shoots grow from, and the turtle that turns a grown string into stems and
//! organs.
//!
//! The alphabet: `A` the growing tip of a main axis, `B` the tip of a side
//! branch, `I` an internode, `[` and `]` a side growth, `+` and `-` turns to
//! the active side and the other, `!` swaps the active side, `/` kinks the
//! axis, and `L U K T C` are a leaf, a flower, a fruit, a thorn and a tendril.

use crate::body::{outline_at, ApexStyle, Body, Kind};
use crate::face::Face;
use crate::maths::{clamp, grounded, hyp, lerp, max, min, rot, toward, unit, DEG, DOWN, UP, V};
use crate::mood::Mood;
use crate::reader::{dna, Reader, Stream};

const SIDE_GROWTH: [&str; 6] = ["[+L]", "[+B]", "[+U]", "[+K]", "[+C]", "[+T]"];
const SIDE_WEIGHTS: [u32; 6] = [7, 3, 1, 1, 1, 1];

/// The growth rules: two for each kind of tip, how often (out of 256) a tip
/// takes its second rule, and what closes a tip still open when growth stops.
#[derive(Clone, Debug)]
pub(crate) struct Rules {
    a: [String; 2],
    b: [String; 2],
    mix_a: u8,
    mix_b: u8,
    end_a: &'static str,
    end_b: &'static str,
}

fn main_rule(r: &Reader<'_>, at: usize) -> String {
    let s1 = SIDE_GROWTH[r.pick(at, &SIDE_WEIGHTS)];
    // The other side of the node: nothing, the same again, a leaf or a
    // thorn. A node never carries a flower or a fruit on both sides.
    let k = r.pick(at + 1, &[6, 4, 2, 1]);
    let s2 = if k == 0 || (k == 1 && (s1 == "[+U]" || s1 == "[+K]")) {
        String::new()
    } else if k == 1 {
        s1.replacen('+', "-", 1)
    } else if k == 2 {
        "[-L]".to_owned()
    } else {
        "[-T]".to_owned()
    };
    let swap = if s2.is_empty() && r.chance(at + 2, 0.8) {
        "!"
    } else {
        ""
    };
    format!(
        "I{s1}{s2}{swap}{}",
        ["A", "/A", "[+A][-A]"][r.pick(at + 3, &[6, 2, 2])]
    )
}

fn side_rule(r: &Reader<'_>, at: usize) -> String {
    format!(
        "I{}{}",
        ["[+L]", "[+L][-L]", "[+U]", "[+K]", "[+T]", ""][r.pick(at, &[5, 2, 2, 1, 1, 2])],
        if r.chance(at + 1, 0.66) { "!B" } else { "B" }
    )
}

pub(crate) fn build_rules(r: &Reader<'_>) -> Rules {
    let o = dna::RULES;
    Rules {
        a: [main_rule(r, o), main_rule(r, o + 4)],
        b: [side_rule(r, o + 9), side_rule(r, o + 11)],
        mix_a: [0, 40, 84, 128][r.pick(o + 8, &[3, 2, 2, 1])],
        mix_b: [0, 48, 96][r.pick(o + 13, &[3, 2, 1])],
        end_a: ["L", "U", "[+L][-L]", "[+L][-L]U", "K", "C", "T", ""]
            [r.pick(o + 14, &[8, 8, 4, 4, 2, 2, 2, 1])],
        end_b: ["L", "U", "K", "C", "T", ""][r.pick(o + 15, &[10, 6, 4, 2, 2, 2])],
    }
}

/// How many characters of `s` are among `chars`.
pub(crate) fn count(s: &str, chars: &str) -> usize {
    s.chars().filter(|c| chars.contains(*c)).count()
}

/// Where the bracket that opens at `i` ends: just past its closing bracket.
fn bracket_end(s: &[u8], i: usize) -> usize {
    let mut depth = 1_i32;
    let mut j = i + 1;
    while j < s.len() && depth != 0 {
        depth += match s[j] {
            b'[' => 1,
            b']' => -1,
            _ => 0,
        };
        j += 1;
    }
    j
}

/// The same shoot without growth that repeats at one node.
fn single(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::new();
    let mut seen: Vec<String> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'[' {
            out.push(bytes[i] as char);
            seen.clear();
            i += 1;
            continue;
        }
        let j = bracket_end(bytes, i);
        let inner_end = j.saturating_sub(1).max(i + 2);
        let part = format!(
            "[{}{}]",
            bytes
                .get(i + 1)
                .map_or(String::new(), |&c| (c as char).to_string()),
            single(s.get(i + 2..inner_end).unwrap_or(""))
        );
        if !seen.contains(&part) {
            out.push_str(&part);
            seen.push(part);
        }
        i = j;
    }
    out
}

/// How many stretches of stem the shoot's own axis has.
pub(crate) fn axis_length(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut n = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            i = bracket_end(bytes, i);
            continue;
        }
        if bytes[i] == b'I' {
            n += 1;
        }
        i += 1;
    }
    n
}

/// Grows one tip for `age` generations and closes what is still open. A
/// plant with more organs or stem than `cap` allows is grown again a
/// generation younger.
fn derive(g: &Rules, age: i32, st: &mut Stream, cap: [usize; 2]) -> String {
    let mut generation = age;
    while generation >= 1 {
        st.reset();
        let mut s = "A".to_owned();
        for _ in 0..generation {
            let mut out = String::new();
            for ch in s.chars() {
                match ch {
                    'A' => {
                        let second = g.mix_a != 0 && st.next() < g.mix_a;
                        out.push_str(&g.a[usize::from(second)]);
                    }
                    'B' => {
                        let second = g.mix_b != 0 && st.next() < g.mix_b;
                        out.push_str(&g.b[usize::from(second)]);
                    }
                    _ => out.push(ch),
                }
            }
            s = out;
        }
        let closed = s.replace('A', g.end_a).replace('B', g.end_b);
        // A side tip that closes with nothing leaves an empty side growth.
        let closed = single(&closed.replace("[+]", "").replace("[-]", ""));
        if count(&closed, "LUKTC") <= cap[0] && count(&closed, "I") <= cap[1] {
            return closed;
        }
        // Even one generation can come out too heavy; then every tip ends
        // in a single leaf instead.
        if generation == 1 {
            return single(&s.replace(['A', 'B'], "L"));
        }
        generation -= 1;
    }
    "IL".to_owned()
}

/// The same shoot with every side branch replaced by a single leaf.
fn unbranched(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'[' {
            out.push(bytes[i] as char);
            i += 1;
            continue;
        }
        let j = bracket_end(bytes, i);
        let inner = s.get(i + 1..j.saturating_sub(1).max(i + 1)).unwrap_or("");
        out.push('[');
        if inner.contains('I') {
            out.push_str(&inner[..1]);
            out.push('L');
        } else {
            out.push_str(inner);
        }
        out.push(']');
        i = j;
    }
    out
}

/// Keeps only the last `keep` flowers and fruit; the rest become leaves.
fn thinned(s: &str, keep: usize) -> String {
    let mut seen = 0;
    let mut out: Vec<char> = s.chars().collect();
    for c in out.iter_mut().rev() {
        if *c == 'U' || *c == 'K' {
            seen += 1;
            if seen > keep {
                *c = 'L';
            }
        }
    }
    out.into_iter().collect()
}

/// At most one flower, the one nearest the tip; any other becomes a leaf.
fn one_flower(s: &str) -> String {
    let mut seen = 0;
    let mut out: Vec<char> = s.chars().collect();
    for c in out.iter_mut().rev() {
        if *c == 'U' {
            seen += 1;
            if seen > 1 {
                *c = 'L';
            }
        }
    }
    if seen > 1 {
        single(&out.into_iter().collect::<String>())
    } else {
        s.to_owned()
    }
}

/// A leaf wherever the shoot bore a flower.
pub(crate) fn flowerless(s: &str) -> String {
    if s.contains('U') {
        single(&s.replace('U', "L"))
    } else {
        s.to_owned()
    }
}

/// The same stem bearing leaves only: its lowest side growth as a leaf, and
/// a leaf at its tip.
fn tipped(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::new();
    let mut sides = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            if sides == 0 {
                out.push_str(if bytes.get(i + 1) == Some(&b'-') {
                    "[-L]"
                } else {
                    "[+L]"
                });
            }
            sides += 1;
            i = bracket_end(bytes, i);
            continue;
        }
        if !b"LUKTC".contains(&bytes[i]) {
            out.push(bytes[i] as char);
        }
        i += 1;
    }
    out.push('L');
    out
}

/// The shoot reduced to single leaves, keeping the flower at its tip.
pub(crate) fn singled(s: &str) -> String {
    let t = tipped(s);
    if s.ends_with('U') {
        format!("{}U", &t[..t.len() - 1])
    } else {
        t
    }
}

/// The same shoot with a leaf at its lowest node.
fn leafed(s: &str) -> String {
    let bytes = s.as_bytes();
    let Some(i) = s.find('[') else {
        return s.replacen('I', "I[+L]", 1);
    };
    let j = bracket_end(bytes, i);
    let other = if bytes.get(i + 1) == Some(&b'-') {
        '+'
    } else {
        '-'
    };
    let rest = if bytes.get(j) == Some(&b'[') {
        bracket_end(bytes, j)
    } else {
        j
    };
    format!("{}[{other}L]{}", &s[..j], &s[rest.min(s.len())..])
}

/// How the turtle draws what the rules grow.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Habit {
    pub(crate) len: f64,
    pub(crate) decay: f64,
    pub(crate) lateral: f64,
    pub(crate) branch: f64,
    pub(crate) leaf_angle: f64,
    pub(crate) bend: f64,
    pub(crate) kink: f64,
    pub(crate) trop: f64,
    pub(crate) width: f64,
    pub(crate) light: f64,
    pub(crate) gradient: f64,
    pub(crate) order_decay: f64,
}

pub(crate) fn build_habit(r: &Reader<'_>) -> Habit {
    let o = dna::HABIT;
    Habit {
        len: r.lin(o, 14.0, 25.0),
        decay: r.lin(o + 1, 0.78, 1.0),
        lateral: r.lin(o + 2, 0.6, 0.9),
        branch: r.lin(o + 3, 28.0 * DEG, 72.0 * DEG),
        leaf_angle: r.lin(o + 4, 32.0 * DEG, 82.0 * DEG),
        bend: r.lin(o + 5, -13.0 * DEG, 13.0 * DEG),
        kink: r.lin(o + 6, 12.0 * DEG, 34.0 * DEG),
        trop: r.lin(o + 7, -0.1, 0.34),
        width: r.lin(o + 8, 7.0, 12.5),
        light: r.lin(o + 9, 0.1, 0.55),
        gradient: r.lin(o + 10, -0.3, 0.3),
        order_decay: r.lin(o + 11, 0.7, 0.92),
    }
}

/// The places shoots grow from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SiteName {
    Apex,
    Stalk,
    Ear,
    Arm,
    Tail,
}

impl SiteName {
    pub(crate) fn name(self) -> &'static str {
        match self {
            SiteName::Apex => "apex",
            SiteName::Stalk => "stalk",
            SiteName::Ear => "ear",
            SiteName::Arm => "arm",
            SiteName::Tail => "tail",
        }
    }
}

/// One shoot: where it grows, how, and what it grew.
#[derive(Clone, Debug)]
pub(crate) struct Site {
    pub(crate) name: SiteName,
    pub(crate) pair: bool,
    pub(crate) weld: bool,
    pub(crate) stalk: bool,
    pub(crate) cactus: bool,
    pub(crate) fixed: bool,
    pub(crate) root: V,
    pub(crate) root_l: Option<V>,
    pub(crate) dir: V,
    pub(crate) fall: f64,
    pub(crate) w0: f64,
    pub(crate) age: i32,
    pub(crate) scale: f64,
    pub(crate) flex: f64,
    pub(crate) give: f64,
    pub(crate) trop: Option<f64>,
    pub(crate) tip_k: f64,
    pub(crate) leaf_angle: f64,
    pub(crate) cap: [usize; 2],
    pub(crate) stream: Stream,
    pub(crate) reach: f64,
    pub(crate) ceil: f64,
    pub(crate) stem_ceil: Option<f64>,
    pub(crate) len_min: f64,
    pub(crate) curve: f64,
    pub(crate) len_abs: Option<f64>,
    pub(crate) organ_k: f64,
    pub(crate) lead: f64,
    pub(crate) string: String,
    pub(crate) asym: f64,
    pub(crate) tilt: f64,
    pub(crate) late: usize,
    pub(crate) bunch_k: Vec<Option<f64>>,
    pub(crate) held_k: Option<f64>,
}

impl Site {
    fn new(name: SiteName, root: V, dir: V, stream: Stream) -> Self {
        Self {
            name,
            pair: false,
            weld: false,
            stalk: false,
            cactus: false,
            fixed: false,
            root,
            root_l: None,
            dir,
            fall: 1.0,
            w0: 0.0,
            age: 1,
            scale: 1.0,
            flex: 0.0,
            give: 0.0,
            trop: None,
            tip_k: 0.45,
            leaf_angle: 9.0,
            cap: [0, 0],
            stream,
            reach: 0.0,
            ceil: 0.0,
            stem_ceil: None,
            len_min: 0.0,
            curve: 1.0,
            len_abs: None,
            organ_k: 1.0,
            lead: 0.0,
            string: String::new(),
            asym: 0.0,
            tilt: 0.0,
            late: 0,
            bunch_k: Vec::new(),
            held_k: None,
        }
    }

    /// Whether the shoot drawn on side `m` flowers late.
    pub(crate) fn late_on(&self, m: f64) -> bool {
        (self.late == 1 && m > 0.0) || (self.late == 2 && m < 0.0)
    }
}

/// One shoot's string, grown from the rules and held to what its place on
/// the body allows.
fn grow(g: &Rules, site: &mut Site) -> String {
    let mut s = derive(g, site.age, &mut site.stream, site.cap);
    // The shoot on top of a body is a single stem.
    if site.name == SiteName::Apex {
        s = unbranched(&s);
    }
    // An ear is a single green stem with a leaf at its tip and at most one
    // below it, on at least two stretches of stem.
    if site.name == SiteName::Ear {
        s = tipped(&s);
        if axis_length(&s) < 2 {
            s = format!("{}IL", &s[..s.len() - 1]);
        }
    }
    // At most one flower, and at most two flowers and fruit together (one on
    // the top shoot).
    s = one_flower(&thinned(
        &s,
        if site.name == SiteName::Apex { 1 } else { 2 },
    ));
    // Every shoot the rules grow bears at least one leaf.
    if !s.contains('L') {
        s = leafed(&s);
    }
    // An arm that grew a single stretch of stem gets a bare one below it.
    if site.name == SiteName::Arm && axis_length(&s) < 2 {
        format!("I{s}")
    } else {
        s
    }
}

/// Lays out the shoots: where they grow and what the rules make of them.
pub(crate) fn build_sites(
    r: &Reader<'_>,
    b: &Body,
    f: &Face,
    g: &Rules,
    shy: bool,
    lone: bool,
) -> Vec<Site> {
    let o = dna::PLAN;
    let k = crate::body::spec(b.kind);
    let mut sites = Vec::new();
    let top = b.apex_chance >= 1.0 || (b.apex_chance > 0.0 && r.chance(o, b.apex_chance));
    let trunk = top && k.apex_style == ApexStyle::Trunk;
    // Ears grow only on a body with nothing else on top.
    let mut pair = if b.arm.is_some() && b.ear.is_some() && !top {
        if r.chance(o + 2, 0.3) {
            Some(SiteName::Ear)
        } else {
            Some(SiteName::Arm)
        }
    } else if b.arm.is_some() {
        Some(SiteName::Arm)
    } else if b.ear.is_some() && !top {
        Some(SiteName::Ear)
    } else {
        None
    };
    if pair.is_some()
        && (trunk || b.kind == Kind::Rosette)
        && !r.chance(o + 1, if trunk { 0.75 } else { 0.5 })
    {
        pair = None;
    }
    let stream = |slot: usize| r.stream(dna::STREAMS + 32 * slot, 32);
    if trunk {
        let lean = r.sgn(o + 7) * 0.22;
        let mut s = Site::new(
            SiteName::Apex,
            [0.0, b.half[0].y + 5.0],
            rot(UP, lean),
            stream(0),
        );
        s.weld = true;
        s.fall = if lean < 0.0 { -1.0 } else { 1.0 };
        s.w0 = b.apex_w;
        s.age = 1 + r.one(o + 5, 2) as i32;
        s.scale = r.lin(o + 6, 0.85, 1.2);
        s.flex = 0.24;
        s.give = 0.7;
        s.trop = Some(0.0);
        s.tip_k = 0.3;
        s.leaf_angle = 0.9;
        s.cap = [4, 2];
        s.reach = b.w * 0.8;
        s.ceil = b.top - b.h * if b.kind == Kind::Bulb { 0.55 } else { 0.5 };
        sites.push(s);
    } else if top {
        // The stalk a fruit hung from, with a tendril or a leaf at its foot.
        let lean = r.sgn(o + 7) * 0.4;
        let mut s = Site::new(
            SiteName::Stalk,
            [0.0, b.half[0].y + 5.0],
            rot(UP, lean),
            stream(0),
        );
        s.stalk = true;
        s.fall = if lean < 0.0 { -1.0 } else { 1.0 };
        s.w0 = b.apex_w;
        s.fixed = true;
        s.string = ["I[+C]", "I[+L]", "I[+C][-L]", "I[+L][-C]", "I", "I[+C]I"]
            [r.pick(o + 5, &[3, 3, 2, 2, 1, 1])]
        .to_owned();
        s.age = 1;
        s.scale = r.lin(o + 6, 0.7, 1.0);
        s.flex = 0.2;
        s.give = 0.5;
        s.trop = Some(0.0);
        s.tip_k = 0.8;
        s.cap = [4, 3];
        s.reach = b.w * 0.7;
        s.ceil = b.top - b.h * 0.42;
        s.organ_k = 0.78;
        sites.push(s);
    }
    if pair == Some(SiteName::Ear) {
        let ear = b.ear.expect("an ear pair has an ear band");
        let at = outline_at(b, lerp(ear[0], ear[1], r.unit(o + 12)));
        let w0 = r.lin(o + 9, 10.0, 16.0);
        let mut s = Site::new(
            SiteName::Ear,
            [at.p[0] - at.n[0] * w0 * 0.5, at.p[1] - at.n[1] * w0 * 0.5],
            unit(at.n[0] * 0.55, at.n[1] * 0.55 - 0.6),
            stream(1),
        );
        s.pair = true;
        s.weld = true;
        s.w0 = w0;
        s.tip_k = 0.3;
        s.age = 1 + r.one(o + 13, 2) as i32;
        s.scale = r.lin(o + 14, 0.65, 0.9);
        s.flex = 0.6;
        s.give = 1.0;
        s.cap = [3, 2];
        s.reach = b.w * 0.7;
        s.ceil = b.top - max(b.h * 0.5, b.w * 0.84);
        s.len_min = w0 * 0.6 + b.w * 0.09;
        s.curve = 0.25;
        s.leaf_angle = 0.65;
        sites.push(s);
    } else if pair == Some(SiteName::Arm) {
        let arm = b.arm.expect("an arm pair has an arm band");
        // The arms leave the body below the eyes, where shoulders would be.
        let y1 = min(max(arm[1], f.ey + f.ry + b.h * 0.1), b.bottom - b.h * 0.25);
        let y0 = min(max(arm[0], f.ey + f.ry), y1);
        let at = outline_at(b, lerp(y0, y1, r.unit(o + 12)));
        let size = r.lin(o + 14, 0.75, 1.05);
        if b.kind == Kind::Column && r.chance(dna::EXTRA, 0.62) {
            // A cactus column grows thick arms that set out sideways and
            // turn up beside it, one set higher than the other.
            let thick = b.w * r.lin(dna::EXTRA + 1, 0.5, 0.66);
            let drop = r.sgn(dna::EXTRA + 3) * 0.1 * b.h;
            let other = outline_at(b, clamp(at.p[1] + drop, y0, y1 + b.h * 0.08));
            let mut s = Site::new(
                SiteName::Arm,
                [at.p[0] - at.n[0] * 6.0, at.p[1] - at.n[1] * 6.0],
                rot(at.n, -0.15),
                stream(1),
            );
            s.pair = true;
            s.weld = true;
            s.cactus = true;
            s.root_l = Some([
                -(other.p[0] - other.n[0] * 6.0),
                other.p[1] - other.n[1] * 6.0,
            ]);
            s.w0 = thick;
            s.tip_k = 0.82;
            s.len_abs = Some(thick * 0.8);
            s.fixed = true;
            s.string = format!(
                "III{}",
                ["", "U", "T", "[+T][-T]", "K"][r.pick(dna::EXTRA + 2, &[1, 5, 1, 1, 1])]
            );
            s.age = 3;
            s.scale = size;
            s.trop = Some(1.7);
            s.flex = 0.3;
            s.give = 0.35;
            s.cap = [5, 4];
            s.reach = b.w * 1.1;
            s.ceil = b.top + b.h * 0.08;
            sites.push(s);
        } else {
            let w0 = r.lin(o + 9, 13.0, 21.0);
            // A body wider than it is tall has no room beside it.
            let squat = clamp(1.0 - b.h / (2.0 * b.w), 0.0, 0.5);
            let mut s = Site::new(
                SiteName::Arm,
                [at.p[0] - at.n[0] * w0 * 0.5, at.p[1] - at.n[1] * w0 * 0.5],
                rot([1.0, 0.0], -r.lin(o + 15, 0.0, 0.6)),
                stream(1),
            );
            s.pair = true;
            s.weld = true;
            s.w0 = w0;
            s.tip_k = 0.3;
            s.age = 1 + r.one(o + 13, 3) as i32;
            s.scale = size * b.arm_scale.unwrap_or(1.0);
            s.organ_k = if b.arm_scale.is_some() { 0.82 } else { 1.0 };
            s.flex = 0.9;
            s.give = 1.0;
            s.cap = [5, 4];
            s.reach = b.w * (0.9 - squat * 0.8);
            // At rest the stems rise no higher than the middle of the eyes,
            // and the leaves on them no higher than the tops of the eyes.
            s.ceil = f.ey - f.ry * 1.3;
            s.stem_ceil = Some(max(at.p[1] - b.h * 0.16, f.ey));
            sites.push(s);
        }
    }
    // A tail on one side only: a runner with a plantlet, a tendril, or a
    // flowering spike.
    let tail = r.pick(o + 20, &[8, 1, 1, 1]);
    if tail != 0 {
        let side = if r.chance(o + 21, 0.5) { 1.0 } else { -1.0 };
        let at = outline_at(b, b.top + b.h * 0.8);
        let mut s = Site::new(
            SiteName::Tail,
            [side * (at.p[0] - at.n[0] * 5.0), at.p[1] - at.n[1] * 5.0],
            rot([side * at.n[0], at.n[1]], side * 0.35),
            stream(2),
        );
        s.fall = 1.0;
        s.w0 = 5.5;
        s.fixed = true;
        s.string = ["", "II[+L][-L]L", "IC", "IIU"][tail].to_owned();
        s.age = 1;
        s.scale = r.lin(o + 22, 0.6, 0.9);
        s.flex = 0.8;
        s.give = 1.1;
        s.trop = Some(0.3);
        s.cap = [4, 3];
        s.reach = b.w * 0.6;
        s.ceil = at.p[1] - b.h * 0.3;
        sites.push(s);
    }
    for s in sites.iter_mut() {
        if !s.fixed {
            s.string = grow(g, s);
        }
    }
    let organs = |s: &Site| count(&s.string, "LUKTC") * if s.pair { 2 } else { 1 };
    let stems = |s: &Site| count(&s.string, "I") * if s.pair { 2 } else { 1 };
    // No creature carries more than 16 organs in all: past that, the shoot
    // with the most is grown a generation younger.
    for _ in 0..8 {
        if sites.iter().map(organs).sum::<usize>() <= 16 {
            break;
        }
        let mut most: Option<usize> = None;
        for (i, s) in sites.iter().enumerate() {
            if !s.fixed && s.age > 1 && most.is_none_or(|m| organs(s) > organs(&sites[m])) {
                most = Some(i);
            }
        }
        let Some(i) = most else { break };
        sites[i].age -= 1;
        sites[i].string = grow(g, &mut sites[i]);
    }
    // Nor is growth a token gesture: a creature whose shoots come to fewer
    // than ten organs and stretches of stem grows its youngest shoot older.
    for _ in 0..2 {
        if sites.is_empty()
            || sites.iter().map(organs).sum::<usize>() + sites.iter().map(stems).sum::<usize>()
                >= 10
        {
            break;
        }
        let mut least: Option<usize> = None;
        for (i, s) in sites.iter().enumerate() {
            if !s.fixed && s.age < 3 && least.is_none_or(|m| s.age < sites[m].age) {
                least = Some(i);
            }
        }
        let Some(i) = least else { break };
        sites[i].age += 1;
        sites[i].string = grow(g, &mut sites[i]);
    }
    // Flowers open on the top shoot or on the pair, never on both.
    let crown = sites.iter().position(|s| s.name == SiteName::Apex);
    let flanks = sites.iter().position(|s| s.pair);
    if let Some(fl) = flanks {
        if lone && sites[fl].name == SiteName::Arm {
            sites[fl].string = if sites[fl].cactus {
                "III".to_owned()
            } else {
                singled(&sites[fl].string)
            };
        }
    }
    if let (true, Some(fl)) = (shy, flanks) {
        sites[fl].string = if sites[fl].cactus {
            sites[fl].string.replace('U', "")
        } else {
            flowerless(&sites[fl].string)
        };
    } else if let (Some(cr), Some(fl)) = (crown, flanks) {
        if count(&sites[cr].string, "U") > 0 && count(&sites[fl].string, "U") > 0 {
            let bare = if r.chance(o + 3, 0.5) { fl } else { cr };
            sites[bare].string = flowerless(&sites[bare].string);
        }
    }
    // The two of a pair are the same shoot mirrored, then told apart.
    for s in sites.iter_mut() {
        if !s.pair {
            continue;
        }
        s.asym = r.sgn(o + 24) * 0.13;
        s.tilt = if s.cactus { 0.0 } else { r.sgn(o + 28) * 0.16 };
        s.late = if count(&s.string, "U") > 0 {
            r.pick(o + 30, &[5, 1, 1])
        } else {
            0
        };
    }
    sites
}

/// One stem: its points with their widths, the axis it branches from and
/// the index of the node there.
#[derive(Clone, Debug)]
pub(crate) struct Axis {
    pub(crate) pts: Vec<[f64; 3]>,
    pub(crate) from: Option<usize>,
    pub(crate) from_at: usize,
}

/// One organ the turtle placed.
#[derive(Clone, Debug)]
pub(crate) struct Organ {
    pub(crate) kind: char,
    pub(crate) p: V,
    /// Which node of the shoot the organ grows from: organs at one node
    /// share it.
    pub(crate) node: usize,
    pub(crate) h: V,
    pub(crate) stem: V,
    pub(crate) sign: f64,
    pub(crate) on: Option<usize>,
    pub(crate) at: usize,
    pub(crate) size: f64,
    pub(crate) late: bool,
}

/// What one shoot draws in one mood.
pub(crate) struct Grown {
    pub(crate) axes: Vec<Axis>,
    pub(crate) organs: Vec<Organ>,
}

#[derive(Clone)]
struct Turtle {
    p: V,
    node: usize,
    h: V,
    side: f64,
    order: i32,
    k: i32,
    len: f64,
    axis: Option<usize>,
    from: Option<usize>,
    from_at: usize,
    curl: f64,
    turn: f64,
    stem: V,
    base: i32,
}

/// Draws one grown string as stems and organs. `m` is +1 for a shoot as its
/// site gives it and -1 for its mirror image.
pub(crate) fn interpret(hb: &Habit, site: &Site, m: f64, mood: &Mood) -> Grown {
    let s = site.string.as_bytes();
    let scale = site.scale * if site.pair { 1.0 + m * site.asym } else { 1.0 };
    let p = if m < 0.0 {
        site.root_l.unwrap_or([-site.root[0], site.root[1]])
    } else {
        site.root
    };
    let mut h = if m < 0.0 {
        [-site.dir[0], site.dir[1]]
    } else {
        site.dir
    };
    if site.pair && m > 0.0 && site.tilt != 0.0 {
        h = rot(h, -site.tilt);
    }
    // The whole shoot gives way at its base, then each stretch a little more.
    let fall = m * site.fall;
    if mood.droop > 0.0 {
        h = toward(h, DOWN, mood.droop * site.flex * 0.95, fall);
    } else if mood.droop < 0.0 {
        h = toward(h, UP, -mood.droop * site.flex * 0.6, fall);
    }
    if mood.lean != 0.0 {
        h = rot(h, mood.lean * 0.16 * site.flex);
    }
    let trop = site.trop.unwrap_or(hb.trop);
    let mut axes: Vec<Axis> = Vec::new();
    let mut organs: Vec<Organ> = Vec::new();
    let mut stack: Vec<Turtle> = Vec::new();
    let mut nodes = 0;
    let mut st = Turtle {
        p,
        node: 0,
        h,
        side: 1.0,
        order: 0,
        k: 0,
        len: site.len_abs.unwrap_or(hb.len) * scale,
        axis: None,
        from: None,
        from_at: 0,
        curl: 1.0,
        turn: m,
        stem: h,
        base: 0,
    };
    for i in 0..s.len() {
        let ch = s[i];
        match ch {
            b'I' => {
                let axis = match st.axis {
                    Some(axis) => axis,
                    None => {
                        axes.push(Axis {
                            pts: vec![[st.p[0], st.p[1], 0.0]],
                            from: st.from,
                            from_at: st.from_at,
                        });
                        st.axis = Some(axes.len() - 1);
                        axes.len() - 1
                    }
                };
                // An ear's own stem keeps a least length for each stretch.
                let len = max(
                    st.len * if st.order != 0 { hb.lateral } else { 1.0 },
                    if st.order != 0 { 0.0 } else { site.len_min },
                ) + if st.order != 0 || st.k != 0 {
                    0.0
                } else {
                    site.lead
                };
                for _ in 0..2 {
                    let give = min(1.3, 0.35 + 0.3 * f64::from(st.k))
                        * if st.order != 0 { 1.2 } else { 1.0 }
                        * site.give;
                    let mut hh = rot(
                        st.h,
                        m * st.curl * hb.bend * if site.cactus { 0.2 } else { site.curve },
                    );
                    // Growth turns up towards the light, or weeps; a cactus
                    // arm sets out sideways before it turns up.
                    let pull = mood.droop * give * 0.42
                        - (if st.order != 0 { hb.trop } else { trop })
                            * 0.25
                            * if site.cactus && st.k == 0 { 0.5 } else { 1.0 };
                    hh = if pull > 0.0 {
                        toward(hh, DOWN, pull, fall)
                    } else {
                        toward(hh, UP, -pull, fall)
                    };
                    if mood.lean != 0.0 {
                        hh = rot(hh, mood.lean * 0.07 * give);
                    }
                    // A shoot that reaches the ground runs along it.
                    hh = grounded(st.p, hh, len / 2.0, mood.floor - 4.0);
                    st.h = hh;
                    st.p = [st.p[0] + hh[0] * len / 2.0, st.p[1] + hh[1] * len / 2.0];
                    nodes += 1;
                    st.node = nodes;
                    axes[axis].pts.push([st.p[0], st.p[1], 0.0]);
                }
                st.k += 1;
                st.len *= hb.decay;
                st.stem = st.h;
            }
            b'[' => {
                let parent = st.clone();
                st.from = parent.axis;
                st.from_at = parent.axis.map_or(0, |a| axes[a].pts.len() - 1);
                st.axis = None;
                st.order = parent.order + 1;
                st.stem = parent.h;
                st.base = parent.order;
                stack.push(parent);
            }
            b']' => {
                st = stack.pop().expect("brackets in a grown string balance");
            }
            b'+' | b'-' => {
                let next = s.get(i + 1).copied();
                let sign = (if ch == b'+' { 1.0 } else { -1.0 }) * st.side * m;
                let angle = match next {
                    Some(b'L') => min(hb.leaf_angle, site.leaf_angle) * mood.spread,
                    Some(b'T') => 1.3,
                    Some(b'C') => 0.9,
                    _ => {
                        hb.branch
                            * if next == Some(b'U') || next == Some(b'K') {
                                0.8
                            } else {
                                1.0
                            }
                            * mood.spread
                    }
                };
                st.h = rot(st.h, sign * min(angle, 1.75));
                st.curl = (if ch == b'+' { 1.0 } else { -1.0 }) * st.side;
                st.turn = sign;
            }
            b'!' => st.side = -st.side,
            b'/' => {
                st.h = rot(st.h, m * st.side * hb.kink * site.curve);
            }
            b'L' | b'U' | b'K' | b'T' | b'C' => {
                // An organ sits on the side of the axis below it, or at the
                // tip of its own axis.
                let tip = st.axis.is_some() || stack.is_empty();
                let on = if tip { st.axis } else { st.from };
                let order = if tip { st.order } else { st.base };
                let mut size = (0.55 + 0.45 * scale)
                    * site.organ_k
                    * clamp(1.0 + hb.gradient * (f64::from(st.k) / 3.0 - 0.5), 0.72, 1.3);
                for _ in 0..order {
                    size *= hb.order_decay;
                }
                // One byte of the shoot's own stream for each organ: its upper
                // half turns the organ a little, its lower half sizes it.
                let own = site.stream.at(12 + organs.len());
                let oh = rot(st.h, m * (f64::from(own >> 4) - 7.5) / 7.5 * 0.2);
                size *= 0.87 + 0.26 * f64::from(own & 15) / 15.0;
                if ch == b'L' {
                    if let Some(Some(k)) = site.bunch_k.get(organs.len()) {
                        size *= k;
                    }
                }
                if ch == b'U' || ch == b'K' {
                    if let Some(k) = site.held_k {
                        size *= k;
                    }
                }
                organs.push(Organ {
                    kind: ch as char,
                    p: st.p,
                    node: st.node,
                    h: oh,
                    stem: if tip { st.h } else { st.stem },
                    sign: if tip { m } else { st.turn },
                    on,
                    at: match on {
                        None => 0,
                        Some(a) if tip => axes[a].pts.len() - 1,
                        Some(_) => st.from_at,
                    },
                    size,
                    late: false,
                });
            }
            _ => {}
        }
    }
    // Widths: a welded trunk flares at its foot like a tree; other stems
    // taper evenly.
    for index in 0..axes.len() {
        let mut total = 0.0;
        let mut cum = vec![0.0];
        {
            let pts = &axes[index].pts;
            for i in 1..pts.len() {
                total += hyp(pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1]);
                cum.push(total);
            }
        }
        let from = axes[index].from;
        let base = match from {
            Some(parent) => min(axes[parent].pts[axes[index].from_at][2] * 0.74, hb.width),
            None => site.w0 * if site.weld { 1.0 } else { 0.6 + 0.4 * scale },
        };
        let end = max(3.2, base * site.tip_k);
        let flare = site.weld && from.is_none() && !site.cactus;
        for (i, p) in axes[index].pts.iter_mut().enumerate() {
            let t = if total != 0.0 { cum[i] / total } else { 0.0 };
            p[2] = if flare {
                end + (base - end) * (1.0 - t) * (1.0 - t)
            } else {
                lerp(base, end, t)
            };
        }
        if !site.weld && from.is_none() {
            axes[index].pts[0][2] *= 1.25;
        }
    }
    Grown { axes, organs }
}
