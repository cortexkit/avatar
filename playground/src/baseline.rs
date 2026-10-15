//! Render registered-identity genomes and their review sheets. The committed
//! pre-port baseline is immutable; later renders must go to a different root.

use super::*;
use std::collections::BTreeSet;
use std::path::Path;

struct Row {
    project_id: String,
    bytes: [u8; genome::GENOME_LEN],
    moods: Vec<mood::Mood>,
}

fn unhex(hex: &str) -> [u8; genome::GENOME_LEN] {
    assert_eq!(hex.len(), genome::GENOME_LEN * 2);
    std::array::from_fn(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("genome hex"))
}

fn selectors(c: &Creature) -> BTreeSet<String> {
    let mut values: BTreeSet<_> = [
        format!("spokes={}", c.wb.spokes.len()),
        format!("pairs={}", c.exts.len()),
        format!("topology={}", c.topology),
        format!("recipe={}", c.recipe),
        format!("eyes={:?}", c.eyes),
        format!("mouth={:?}", c.mouth),
        format!("voronoi={}", c.voronoi),
        format!("belly={}", c.belly_variant),
        format!("brow={}", c.face_style.brow),
    ]
    .into_iter()
    .collect();
    values.extend(c.ext_fills.iter().map(|fill| format!("fill={fill}")));
    values
}

fn required_selectors() -> BTreeSet<String> {
    let mut values = BTreeSet::new();
    for (name, range) in [
        ("spokes", 5..17),
        ("pairs", 0..5),
        ("topology", 0..3),
        ("belly", 0..4),
        ("brow", 0..4),
        ("fill", 0..3),
    ] {
        values.extend(range.map(|value| format!("{name}={value}")));
    }
    for recipe in [0, 2, 3, 4] {
        values.insert(format!("recipe={recipe}"));
    }
    for eye in ["DotShine", "Lidded", "HappyArc", "Uneven", "HalfLid"] {
        values.insert(format!("eyes={eye}"));
    }
    for mouth in ["Arc", "CatW", "OpenHappy", "Snaggle", "Flat", "Fangs"] {
        values.insert(format!("mouth={mouth}"));
    }
    values.extend(["voronoi=true".to_owned(), "voronoi=false".to_owned()]);
    values
}

fn write(path: &Path, contents: impl AsRef<[u8]>) {
    std::fs::write(path, contents)
        .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}

fn freeze(candidates: &Path, out: &Path) {
    assert!(!out.join("corpus.tsv").exists(), "corpus is already frozen");
    let input = std::fs::read_to_string(candidates).expect("read candidate TSV");
    let rows: Vec<_> = input
        .lines()
        .map(|line| {
            let (project_id, hex) = line.split_once('\t').expect("project_id TAB hex");
            let bytes = unhex(hex);
            (project_id, hex, selectors(&decode_genome(&bytes)))
        })
        .collect();
    let mut remaining = required_selectors();
    let mut chosen = Vec::new();
    while !remaining.is_empty() {
        // Input-order ties make selection reproducible without hash iteration.
        let (index, gain) = rows
            .iter()
            .enumerate()
            .map(|(i, (_, _, values))| (i, values.intersection(&remaining).count()))
            .max_by_key(|(i, gain)| (*gain, std::cmp::Reverse(*i)))
            .expect("candidate population");
        assert!(gain > 0, "uncovered selectors: {remaining:?}");
        remaining.retain(|value| !rows[index].2.contains(value));
        chosen.push(index);
    }
    std::fs::create_dir_all(out.join("genomes")).expect("create genome directory");
    let mut manifest =
        String::from("# project_id\tmoods (neutral for all; seven for the first row)\n");
    let mut coverage = String::from("project_id\tdecoded selectors\n");
    for (position, index) in chosen.iter().enumerate() {
        let (id, hex, values) = &rows[*index];
        assert!(id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
        let moods = if position == 0 {
            mood::ALL
                .iter()
                .map(|m| mood::name(*m))
                .collect::<Vec<_>>()
                .join(",")
        } else {
            "neutral".to_owned()
        };
        manifest.push_str(&format!("{id}\t{moods}\n"));
        coverage.push_str(&format!(
            "{id}\t{}\n",
            values.iter().cloned().collect::<Vec<_>>().join(",")
        ));
        write(
            &out.join("genomes").join(format!("{id}.hex")),
            format!("{hex}\n"),
        );
    }
    write(&out.join("corpus.tsv"), manifest);
    write(&out.join("selectors.tsv"), coverage);
    println!(
        "froze {} genomes from {} candidates; all 48 selectors covered",
        chosen.len(),
        rows.len()
    );
}

fn load(root: &Path) -> Vec<Row> {
    std::fs::read_to_string(root.join("corpus.tsv"))
        .expect("read frozen corpus")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let (id, moods) = line.split_once('\t').expect("project_id TAB moods");
            let hex = std::fs::read_to_string(root.join("genomes").join(format!("{id}.hex")))
                .expect("read registered genome");
            Row {
                project_id: id.to_owned(),
                bytes: unhex(hex.trim()),
                moods: moods
                    .split(',')
                    .map(|name| {
                        *mood::ALL
                            .iter()
                            .find(|m| mood::name(**m) == name)
                            .expect("known mood")
                    })
                    .collect(),
            }
        })
        .collect()
}

fn tree(c: &Creature, m: mood::Mood) -> RenderTree {
    RenderTree {
        canvas: 240,
        background: None,
        primitives: draw_full(c, m, 120.0, 120.0),
    }
}

fn sheet_start(w: f64, h: f64) -> String {
    format!("<svg xmlns='http://www.w3.org/2000/svg' width='{w:.0}' height='{h:.0}' viewBox='0 0 {w:.0} {h:.0}'><rect width='100%' height='100%' fill='#141318'/>")
}

fn sheet_cell(
    svg: &mut String,
    c: &Creature,
    m: mood::Mood,
    x: f64,
    y: f64,
    size: f64,
    label: &str,
) {
    svg.push_str(&format!(
        "<rect x='{x}' y='{y}' width='{size}' height='{size}' rx='12' fill='#efe9e4'/>"
    ));
    let scale = size / 240.0;
    svg.push_str(&tree_fragment(
        draw_full(c, m, 120.0, 120.0),
        &[render::translate(x, y), render::scale(scale, scale)],
    ));
    if !label.is_empty() {
        svg.push_str(&format!("<text x='{}' y='{}' font-family='monospace' font-size='13' fill='#8f8a96'>{label}</text>", x + 8.0, y + 20.0));
    }
}

fn mint(root: &Path) {
    assert!(
        !root.join("baseline").exists(),
        "refuse to overwrite a saved baseline"
    );
    let rows = load(root);
    let out = root.join("baseline");
    std::fs::create_dir_all(out.join("trees")).expect("create baseline directory");
    let creatures: Vec<_> = rows.iter().map(|row| decode_genome(&row.bytes)).collect();
    let mut pairs = 0;
    for (row, c) in rows.iter().zip(&creatures) {
        for &m in &row.moods {
            let json = avatar_svg::serialize_tree_json(&tree(c, m));
            write(
                &out.join("trees")
                    .join(format!("{}__{}.json", row.project_id, mood::name(m))),
                &json,
            );
            pairs += 1;
        }
    }
    let mut review = sheet_start(1640.0, 8.0 + rows.len().div_ceil(4) as f64 * 408.0);
    let mut moods = sheet_start(2358.0, 28.0 + rows.len() as f64 * 336.0);
    for (i, (row, c)) in rows.iter().zip(&creatures).enumerate() {
        sheet_cell(
            &mut review,
            c,
            mood::Mood::Neutral,
            8.0 + (i % 4) as f64 * 408.0,
            8.0 + (i / 4) as f64 * 408.0,
            400.0,
            &row.project_id,
        );
        for (j, m) in mood::ALL.iter().enumerate() {
            let x = 6.0 + j as f64 * 336.0;
            if i == 0 {
                moods.push_str(&format!("<text x='{x}' y='16' font-family='monospace' font-size='13' fill='#8f8a96'>{}</text>", mood::name(*m)));
            }
            sheet_cell(
                &mut moods,
                c,
                *m,
                x,
                28.0 + i as f64 * 336.0,
                330.0,
                &row.project_id,
            );
        }
    }
    let mut icons = sheet_start(8.0 + rows.len() as f64 * 56.0, 150.0);
    let mut y = 6.0;
    for size in [28.0, 36.0, 48.0] {
        for (i, c) in creatures.iter().enumerate() {
            sheet_cell(
                &mut icons,
                c,
                mood::Mood::Neutral,
                6.0 + i as f64 * 56.0,
                y,
                size,
                "",
            );
        }
        y += size + 10.0;
    }
    for (name, mut svg) in [
        ("review.svg", review),
        ("moods.svg", moods),
        ("icons.svg", icons),
    ] {
        svg.push_str("</svg>");
        write(&out.join(name), svg);
    }
    println!(
        "saved {pairs} untransformed trees and 3 sheets for {} frozen genomes",
        rows.len()
    );
}

pub(super) fn command(args: &[String]) -> bool {
    if let Some(i) = args.iter().position(|arg| arg == "--freeze-corpus") {
        freeze(
            Path::new(args.get(i + 1).expect("candidate TSV")),
            Path::new(args.get(i + 2).expect("report directory")),
        );
    } else if let Some(i) = args.iter().position(|arg| arg == "--baseline") {
        mint(Path::new(args.get(i + 1).expect("report directory")));
    } else {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/reports/wild-port")
    }

    #[test]
    fn frozen_corpus_covers_region_selectors_and_seven_moods() {
        let rows = load(&root());
        let covered: BTreeSet<_> = rows
            .iter()
            .flat_map(|row| selectors(&decode_genome(&row.bytes)))
            .collect();
        let missing: Vec<_> = required_selectors().difference(&covered).cloned().collect();
        assert!(missing.is_empty(), "missing selectors: {missing:?}");
        assert_eq!(rows.len(), 12);
        assert_eq!(rows.iter().filter(|row| row.moods.len() == 7).count(), 1);
        assert_eq!(rows[0].moods, mood::ALL);
        assert!(rows
            .iter()
            .skip(1)
            .all(|row| row.moods == [mood::Mood::Neutral]));
    }

    #[test]
    fn frozen_corpus_renders_at_every_mood_and_keeps_its_baseline_inventory() {
        let root = root();
        let rows = load(&root);
        let dir = root.join("baseline/trees");
        let mut expected_names = BTreeSet::new();
        for row in &rows {
            for m in mood::ALL {
                let tree = avatar_svg::render_tree_for_bytes(&row.bytes, avatar_svg::WILD, 1, m)
                    .expect("frozen corpus renders through shipped dispatch");
                assert_eq!(tree.canvas, 240);
                assert_eq!(tree.background, None);
                assert!(!tree.primitives.is_empty());
            }
            for &m in &row.moods {
                let filename = format!("{}__{}.json", row.project_id, mood::name(m));
                // These trees came from the pre-port std-trig renderer. Preserve
                // them for comparison with the shipped polynomial renderer.
                assert!(
                    std::fs::metadata(dir.join(&filename))
                        .expect("frozen tree")
                        .len()
                        > 0
                );
                expected_names.insert(filename);
            }
        }
        let actual_names: BTreeSet<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(expected_names.len(), 18);
        assert_eq!(actual_names, expected_names);
    }
}
