//! Committed cross-port conformance vectors for the avatar pipeline.

#[cfg(test)]
pub(crate) mod test_util {
    fn trim_xml_whitespace(text: &str) -> &str {
        text.trim_start_matches([' ', '\t', '\n', '\r'])
    }

    fn name_len(text: &str) -> usize {
        text.bytes()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, b':' | b'-' | b'_' | b'.'))
            .count()
    }

    fn read_attributes<'a>(label: &str, mut text: &'a str) -> (&'a str, bool) {
        loop {
            let trimmed = trim_xml_whitespace(text);
            if let Some(rest) = trimmed.strip_prefix("/>") {
                return (rest, true);
            }
            if let Some(rest) = trimmed.strip_prefix('>') {
                return (rest, false);
            }
            assert!(!trimmed.is_empty(), "{label}: unterminated tag");
            assert!(
                trimmed.len() < text.len(),
                "{label}: expected whitespace before attribute"
            );
            let len = name_len(trimmed);
            assert!(len > 0, "{label}: expected an attribute name");
            let (name, rest) = trimmed.split_at(len);
            let value = trim_xml_whitespace(rest)
                .strip_prefix('=')
                .unwrap_or_else(|| panic!("{label}: expected '=' after {name}"));
            let value = trim_xml_whitespace(value);
            let quote = match value.as_bytes().first() {
                Some(b'\'' | b'"') => value.as_bytes()[0] as char,
                _ => panic!("{label}: unquoted attribute value for {name}"),
            };
            let (contents, rest) = value[1..]
                .split_once(quote)
                .unwrap_or_else(|| panic!("{label}: unterminated attribute value for {name}"));
            assert!(!contents.contains('<'), "{label}: unescaped '<' in {name}");
            text = rest;
        }
    }

    /// Checks the SVG tag stack without adding an XML parser dependency to this
    /// test-only crate. The renderer emits simple XML, so tag balance and quoted
    /// attributes are sufficient smoke coverage for the committed corpus.
    pub(crate) fn assert_well_formed_svg(label: &str, svg: &str) {
        let mut stack = Vec::new();
        let mut index = 0;
        let bytes = svg.as_bytes();
        while index < bytes.len() {
            if bytes[index] != b'<' {
                index += 1;
                continue;
            }
            if svg[index..].starts_with("<!--") {
                let end = svg[index..].find("-->").expect("unterminated comment");
                index += end + 3;
                continue;
            }
            let tag = &svg[index + 1..];
            if let Some(closing) = tag.strip_prefix('/') {
                let (name, rest) = closing.split_once('>').expect("unterminated closing tag");
                let expected = stack.pop().expect("closing tag without an open tag");
                assert_eq!(name.trim(), expected, "{label}: mismatched closing tag");
                index = svg.len() - rest.len();
            } else {
                let (name, attributes) = tag.split_at(name_len(tag));
                assert!(!name.is_empty(), "{label}: empty tag name at byte {index}");
                let (rest, self_closing) = read_attributes(label, attributes);
                if !self_closing {
                    stack.push(name);
                }
                index = svg.len() - rest.len();
            }
        }
        assert!(stack.is_empty(), "{label}: unclosed tags: {stack:?}");
    }

    #[test]
    #[should_panic(expected = "unquoted attribute value")]
    fn svg_smoke_rejects_an_unquoted_attribute() {
        assert_well_formed_svg(
            "bad SVG",
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox=0/>"#,
        );
    }

    #[test]
    fn svg_smoke_accepts_both_quote_styles_and_tag_delimiters_in_values() {
        assert_well_formed_svg("SVG", r#"<svg data-label='>'><g class="/>" /></svg>"#);
    }
}

#[cfg(test)]
mod tree_fixture;

#[cfg(test)]
mod tests {
    mod image;
    mod wild;

    use std::{
        collections::BTreeSet,
        ffi::OsStr,
        fs,
        path::{Path, PathBuf},
        process::Command,
    };

    use avatar_genome::{decode, derive_genome, Genome, SeatIdentity};
    use avatar_phenotype::{from_canonical_json, to_canonical_json, Q32};
    use avatar_svg::{
        build_render_tree, render_svg, serialize_tree, serialize_tree_json, Mood, RenderParams,
    };
    use serde::Deserialize;

    // Revision of the classic conformance fixtures, not generation behavior.
    // CLASSIC_VERSION in crates/avatar/src/dispatch.rs identifies the
    // algorithm used to render stored genomes. Adding fixture cases must not
    // change that version or the appearance rendered for existing identities.
    // This counter changes only when the classic corpus is re-minted.
    const CORPUS_REVISION: u16 = 5;
    const GENOME_VERSION: u16 = 1;
    const GENOME_BYTES: usize = 1_024;

    const NEUTRAL: &[Mood] = &[Mood::Neutral];
    const ALL_MOODS: &[Mood] = &[
        Mood::Neutral,
        Mood::Working,
        Mood::Happy,
        Mood::Waiting,
        Mood::Blocked,
        Mood::Sad,
        Mood::Sleepy,
    ];

    // Including file names and lengths makes this pin sensitive to boundary
    // changes, not only to the concatenated SVG payloads.
    const TREE_SVG_BOUNDARY_SIZE: usize = 1_100_868;
    const TREE_SVG_BOUNDARY_HASH: &str =
        "e32c18f79931b3f69c6f0c81d9ab7f87533059a2924fd5c03450f4f7da134c9b";

    /// Pins for the identity-derived wild corpus: at version 1, neutral for
    /// each genome plus all seven moods for one genome; at version 2, all
    /// seven moods for one genome whose narrowed eyes show where the two
    /// generations differ. Their gradient paint, masked gradients and path
    /// fill rules must survive decoding in Swift and TypeScript, even though
    /// classic trees do not exercise those primitives.
    const WILD_TREE_FIXTURES: &[(&str, usize, &str)] = &[
        (
            "wild-port-0005__neutral.json",
            56904,
            "fa90bdfcabfc9ebe3e30e39e024fcc2b8cca508cef26dae7681fe53ece18f886",
        ),
        (
            "wild-port-0005__working.json",
            57012,
            "a0034668a5519cb077d50ed784c7ae6a64c0f854b1681e38b9d64ce04cb39e8d",
        ),
        (
            "wild-port-0005__happy.json",
            56411,
            "bbf83c1afb7d74be7c76ad830e7deb97d1115c40282b05fad176557474a61df7",
        ),
        (
            "wild-port-0005__waiting.json",
            56488,
            "b5357636eb6b68b49e952dd9bbceaecf2452ecbfd2da4f81df310237b8e5f4e2",
        ),
        (
            "wild-port-0005__blocked.json",
            56546,
            "b61a98138c492268f6bc8dcdce9f9210f0e044d7253be1e4f84eaf7806df631a",
        ),
        (
            "wild-port-0005__sad.json",
            56599,
            "c6230619e7ae57c1d3c00175aea5e7d59e8083589d0b97cbe03c17d6777a4988",
        ),
        (
            "wild-port-0005__sleepy.json",
            58113,
            "a217d9d1eb4b1b5e553b49dd731e75f64c7af1cf8e3fbd6b2adf1e6d34288980",
        ),
        (
            "wild-port-0031__neutral.json",
            22912,
            "bd2c20a0f9ebe1556b20949e50200de1191ef521ce01fff02d48886067f44c70",
        ),
        (
            "wild-port-0009__neutral.json",
            59130,
            "df4ee3339af940f9ab50ce768a110414d271192088567f0d04489504c3e87f5b",
        ),
        (
            "wild-port-0022__neutral.json",
            27747,
            "9066c16923da417cc8ba18f11b5a23bf73596d653cd95879fe5ba31607350b8b",
        ),
        (
            "wild-port-0077__neutral.json",
            35317,
            "ce41c2bb0febd5ca7822c1474ac21d9c43dcc024a7856dcf570d50e1ba489ffe",
        ),
        (
            "wild-port-0043__neutral.json",
            28041,
            "eee196169375294dd48d37be49305cc35cadde4603545102bf62d1c8c1d32c8d",
        ),
        (
            "wild-port-0016__neutral.json",
            41613,
            "3bd9747098e069d38aac997ed2d037e7be5d353721051cabdac0ab0edfc87735",
        ),
        (
            "wild-port-0021__neutral.json",
            55570,
            "fa1f55f7b879e70865b21d044a2bb0a60bc823c412f15add06ab18206c624265",
        ),
        (
            "wild-port-0000__neutral.json",
            50183,
            "db0ae278d392dfbef5e237918c2dfae657b66281612a4a26eceac2024ff0c261",
        ),
        (
            "wild-port-0001__neutral.json",
            27093,
            "7fc5d564bc6e890ebe55c4e6522367fed706c3e7b2f17c87065dbfe691450440",
        ),
        (
            "wild-port-0002__neutral.json",
            54140,
            "2da27f5815d9c62e5c51803f05ac89429e2dd72c2b9935bb68db590fe4738f16",
        ),
        (
            "wild-port-0007__neutral.json",
            20735,
            "c60d1ed53c9b91f498089945f41dffa196265fb8146087dba6e58bbe2b69d42b",
        ),
        (
            "wild-port-0022__v2__neutral.json",
            27747,
            "9066c16923da417cc8ba18f11b5a23bf73596d653cd95879fe5ba31607350b8b",
        ),
        (
            "wild-port-0022__v2__working.json",
            32098,
            "95438955af0faf3c7e1519bc35dc4da11c8ab67715221985e2f3a28c9d9073f0",
        ),
        (
            "wild-port-0022__v2__happy.json",
            30182,
            "b4c6428e187a7e777d77e61da95bad4a951f9c5e8e788aab79ac6fe336a049d2",
        ),
        (
            "wild-port-0022__v2__waiting.json",
            27057,
            "ebfc748483777b695e740ee0da75b04b45269a3087a53d2769e17d52dbed450f",
        ),
        (
            "wild-port-0022__v2__blocked.json",
            29203,
            "020e8719681a4ca96f8fa77811ad4c0ea76ae99f07fb9533dd97093950815093",
        ),
        (
            "wild-port-0022__v2__sad.json",
            29252,
            "a2421a6af7898918d028864b2269566cd4ae6165d997ce23810740fdeccadf35",
        ),
        (
            "wild-port-0022__v2__sleepy.json",
            29995,
            "223072c1cd5a043bf60bf2a08d9758f7bba1ce60bde72ece9733e905eb55dd9d",
        ),
    ];

    struct VectorSpec {
        name: &'static str,
        genome: Genome,
        moods: &'static [Mood],
    }

    #[derive(Deserialize)]
    struct Manifest {
        corpus_revision: u16,
        genome_version: u16,
        producer_sha: String,
        vectors: Vec<ManifestVector>,
    }

    #[derive(Deserialize)]
    struct ManifestVector {
        name: String,
        moods: Vec<String>,
    }

    /// Rewrites committed fixtures only when explicitly requested. Swift and
    /// TypeScript conformance tests consume these bytes; producer_sha records
    /// the Rust source revision that generated them. Routine tests must compare,
    /// not refresh, these byte-level compatibility expectations.
    /// To preserve the classic corpus, refresh only wild fixtures with
    /// `UPDATE_WILD_GOLDEN=1 cargo test -p avatar-vectors tests::mint -- --exact --nocapture --ignored`.
    #[test]
    #[ignore = "set UPDATE_GOLDEN or UPDATE_WILD_GOLDEN to regenerate committed fixtures"]
    fn mint() {
        // A wild-only refresh must leave the classic fixtures and their
        // manifest's recorded source revision unchanged.
        if std::env::var_os("UPDATE_WILD_GOLDEN").is_some() {
            wild::mint_corpus();
            return;
        }
        if !update_golden_enabled() {
            return;
        }
        mint_corpus();
    }

    /// The conformance gate derives every canonical boundary again. Similar-looking
    /// output is not compatible: ports must reproduce the exact phenotype JSON,
    /// tree JSON, and SVG bytes consumed by their vendored corpus.
    #[test]
    fn corpus_is_current() {
        if update_golden_enabled() {
            return;
        }

        assert_current(&contact_sheet_path(), &contact_sheet_bytes());

        for vector in vector_specs() {
            assert_current(&dna_path(vector.name), vector.genome.to_hex().as_bytes());

            let phenotype = decode(&vector.genome);
            assert_current(&phenotype_path(vector.name), &to_canonical_json(&phenotype));

            for &mood in vector.moods {
                let params = render_params(mood);
                let tree = build_render_tree(&phenotype, &params).unwrap_or_else(|error| {
                    panic!(
                        "build tree {} ({}) failed: {error}",
                        vector.name,
                        mood_name(mood)
                    )
                });
                assert_current(
                    &tree_path(vector.name, mood),
                    serialize_tree_json(&tree).as_bytes(),
                );
                let svg = render_svg(&phenotype, &params).unwrap_or_else(|error| {
                    panic!(
                        "render {} ({}) failed: {error}",
                        vector.name,
                        mood_name(mood)
                    )
                });
                assert_current(&svg_path(vector.name, mood), &svg);
            }
        }

        for name in tree_fixture_file_names() {
            let tree = read_tree_fixture(&name);
            assert_current(&tree_svg_fixture_path(&name), &serialize_tree(&tree));
        }
    }

    #[test]
    fn manifest_matches_directory() {
        if update_golden_enabled() {
            return;
        }

        let manifest = read_manifest();
        assert_eq!(manifest.corpus_revision, CORPUS_REVISION);
        assert_eq!(manifest.genome_version, GENOME_VERSION);
        assert_valid_git_sha(&manifest.producer_sha);
        assert_eq!(
            fs::read(manifest_path()).expect("read vectors/manifest.json"),
            manifest_bytes(&manifest.producer_sha),
            "vectors/manifest.json is not canonical; regenerate with UPDATE_GOLDEN=1 cargo test -p avatar-vectors tests::mint -- --ignored"
        );

        let expected_root = BTreeSet::from([
            "contact-sheet.html".to_owned(),
            "dna".to_owned(),
            "drawn-heads".to_owned(),
            "image".to_owned(),
            "knitted-toys".to_owned(),
            "manifest.json".to_owned(),
            "phenotype".to_owned(),
            "pixel-critters".to_owned(),
            "small-robots".to_owned(),
            "sprouts".to_owned(),
            "svg".to_owned(),
            "tree".to_owned(),
            "tree-features".to_owned(),
            "tree-svg".to_owned(),
            "wild".to_owned(),
            "wild-plus".to_owned(),
        ]);
        assert_directory_entry_names("vectors", &expected_root);
        assert!(
            manifest_path().is_file(),
            "vectors/manifest.json is not a file"
        );
        for directory in ["dna", "phenotype", "svg", "tree", "tree-svg", "wild"] {
            assert!(
                corpus_root().join(directory).is_dir(),
                "vectors/{directory} is not a directory"
            );
        }

        let mut expected_dna = BTreeSet::new();
        let mut expected_phenotype = BTreeSet::new();
        let mut expected_svg = BTreeSet::new();
        let mut expected_tree = WILD_TREE_FIXTURES
            .iter()
            .map(|(name, _, _)| (*name).to_owned())
            .collect::<BTreeSet<_>>();
        for vector in &manifest.vectors {
            expected_dna.insert(format!("{}.hex", vector.name));
            expected_phenotype.insert(format!("{}.json", vector.name));
            for mood in &vector.moods {
                expected_svg.insert(format!("{}__{mood}.svg", vector.name));
                expected_tree.insert(format!("{}__{mood}.json", vector.name));
            }
        }
        assert_file_names("dna", &expected_dna);
        assert_file_names("phenotype", &expected_phenotype);
        let expected_tree_svg = expected_tree
            .iter()
            .map(|name| tree_svg_fixture_name(name))
            .collect();
        assert_file_names("svg", &expected_svg);
        assert_file_names("tree", &expected_tree);
        assert_file_names("tree-svg", &expected_tree_svg);
        wild::assert_directory();
    }

    #[test]
    fn phenotype_json_parses() {
        if update_golden_enabled() {
            return;
        }

        for vector in vector_specs() {
            let path = phenotype_path(vector.name);
            let bytes =
                fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            let phenotype = from_canonical_json(&bytes)
                .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
            assert_eq!(
                to_canonical_json(&phenotype),
                bytes,
                "{} did not reserialize to identical canonical bytes",
                path.display()
            );
        }
    }

    #[test]
    fn wild_tree_fixtures_are_hash_pinned() {
        if update_golden_enabled() {
            return;
        }
        for &(name, expected_size, expected_hash) in WILD_TREE_FIXTURES {
            let path = corpus_root().join("tree").join(name);
            let bytes =
                fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert_eq!(bytes.len(), expected_size, "{name} size drifted");
            assert_eq!(
                blake3::hash(&bytes).to_hex().as_str(),
                expected_hash,
                "{name} bytes drifted"
            );
        }
    }

    #[test]
    fn tree_svg_fixtures_are_hash_pinned() {
        if update_golden_enabled() {
            return;
        }

        let mut total_size = 0;
        let mut hasher = blake3::Hasher::new();
        for name in entry_names(&corpus_root().join("tree-svg")) {
            let path = corpus_root().join("tree-svg").join(&name);
            let bytes =
                fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            total_size += bytes.len();
            hasher.update(name.as_bytes());
            hasher.update(&[0]);
            hasher.update(&(bytes.len() as u64).to_le_bytes());
            hasher.update(&bytes);
        }
        assert_eq!(total_size, TREE_SVG_BOUNDARY_SIZE, "tree-svg size drifted");
        assert_eq!(
            hasher.finalize().to_hex().as_str(),
            TREE_SVG_BOUNDARY_HASH,
            "tree-svg bytes drifted"
        );
    }

    #[test]
    fn svg_smoke() {
        if update_golden_enabled() {
            return;
        }

        for vector in vector_specs() {
            for &mood in vector.moods {
                let path = svg_path(vector.name, mood);
                let bytes = fs::read(&path)
                    .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
                let svg = std::str::from_utf8(&bytes)
                    .unwrap_or_else(|error| panic!("{} is not UTF-8: {error}", path.display()));
                crate::test_util::assert_well_formed_svg(&path.display().to_string(), svg);
            }
        }
        for name in tree_fixture_file_names() {
            let path = tree_svg_fixture_path(&name);
            let bytes =
                fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            let svg = std::str::from_utf8(&bytes)
                .unwrap_or_else(|error| panic!("{} is not UTF-8: {error}", path.display()));
            crate::test_util::assert_well_formed_svg(&path.display().to_string(), svg);
        }
    }

    fn mint_corpus() {
        wild::mint_corpus();
        let root = corpus_root();
        for directory in ["dna", "phenotype", "svg", "tree", "tree-svg"] {
            fs::create_dir_all(root.join(directory))
                .unwrap_or_else(|error| panic!("create vectors/{directory}: {error}"));
        }

        for vector in vector_specs() {
            write_corpus_file(&dna_path(vector.name), vector.genome.to_hex().as_bytes());

            let phenotype = decode(&vector.genome);
            write_corpus_file(&phenotype_path(vector.name), &to_canonical_json(&phenotype));

            for &mood in vector.moods {
                let params = render_params(mood);
                let tree = build_render_tree(&phenotype, &params).unwrap_or_else(|error| {
                    panic!(
                        "build tree {} ({}) failed: {error}",
                        vector.name,
                        mood_name(mood)
                    )
                });
                write_corpus_file(
                    &tree_path(vector.name, mood),
                    serialize_tree_json(&tree).as_bytes(),
                );
                let svg = render_svg(&phenotype, &params).unwrap_or_else(|error| {
                    panic!(
                        "render {} ({}) failed: {error}",
                        vector.name,
                        mood_name(mood)
                    )
                });
                write_corpus_file(&svg_path(vector.name, mood), &svg);
            }
        }

        for name in tree_fixture_file_names() {
            let tree = read_tree_fixture(&name);
            write_corpus_file(&tree_svg_fixture_path(&name), &serialize_tree(&tree));
        }

        let producer_sha = producer_sha(&root);
        write_corpus_file(&manifest_path(), &manifest_bytes(&producer_sha));
        write_corpus_file(&contact_sheet_path(), &contact_sheet_bytes());
    }

    /// Renders the visual-review contact sheet the minting flow emits.
    ///
    /// Byte-parity makes every port exactly as right as this corpus, so the
    /// corpus itself must be LOOKED AT before it is committed: a shared
    /// misreading between design note and canonical code produces vectors that
    /// every conformance leg then certifies. Emitting the sheet from the mint
    /// itself keeps that review step visible instead of relying on habit —
    /// open vectors/contact-sheet.html after every re-mint and eyeball each
    /// cell before committing the regeneration.
    fn contact_sheet_bytes() -> Vec<u8> {
        let mut html = String::from(
            "<!doctype html>\n<meta charset=\"utf-8\">\n<title>avatar golden corpus — visual review sheet</title>\n<style>body{font-family:monospace;background:#222;color:#ddd}figure{display:inline-block;margin:6px;text-align:center}img{width:160px;height:160px;background:#fff}figcaption{font-size:11px;margin-top:2px}</style>\n<h1>Golden corpus — review every cell before committing a re-mint</h1>\n",
        );
        for vector in vector_specs() {
            for &mood in vector.moods {
                let file = format!("svg/{}__{}.svg", vector.name, mood_name(mood));
                html.push_str(&format!(
                    "<figure><img src=\"{file}\" alt=\"{file}\"><figcaption>{}<br>{}</figcaption></figure>\n",
                    vector.name,
                    mood_name(mood),
                ));
            }
        }
        html.into_bytes()
    }

    fn contact_sheet_path() -> PathBuf {
        corpus_root().join("contact-sheet.html")
    }

    fn vector_specs() -> Vec<VectorSpec> {
        let mut easing_edges = xof_bytes(b"cortexkit-avatar/vectors/easing-edges/v1");
        set_bits(&mut easing_edges, 0, 4, 2);
        set_bits(&mut easing_edges, 4, 8, 0);
        set_bits(&mut easing_edges, 12, 8, 1);
        set_bits(&mut easing_edges, 20, 8, 127);
        set_bits(&mut easing_edges, 28, 8, 128);
        set_bits(&mut easing_edges, 36, 8, 254);
        set_bits(&mut easing_edges, 44, 8, 255);
        set_bits(&mut easing_edges, 52, 8, 0);
        set_bits(&mut easing_edges, 576 + 8, 8, 0);
        set_bits(&mut easing_edges, 576 + 16, 8, 1);
        set_bits(&mut easing_edges, 576 + 10 * 8, 8, 127);
        set_bits(&mut easing_edges, 576 + 11 * 8, 8, 128);
        set_bits(&mut easing_edges, 576 + 35 * 8, 8, 254);
        set_bits(&mut easing_edges, 576 + 36 * 8, 8, 255);
        set_bits(&mut easing_edges, 576 + 48 * 8, 8, 0);
        set_bits(&mut easing_edges, 576 + 49 * 8, 8, 1);

        let mut sliver = xof_bytes(b"cortexkit-avatar/vectors/sliver/v1");
        set_bits(&mut sliver, 0, 4, 0);
        set_bits(&mut sliver, 4, 8, 255);
        set_bits(&mut sliver, 12, 8, 0);
        set_bits(&mut sliver, 20, 8, 0);
        set_bits(&mut sliver, 36, 8, 0);
        set_bits(&mut sliver, 44, 8, 255);

        vec![
            VectorSpec {
                name: "all_zero",
                genome: Genome::from_bytes([0; GENOME_BYTES]),
                moods: NEUTRAL,
            },
            VectorSpec {
                name: "all_ones",
                genome: Genome::from_bytes([u8::MAX; GENOME_BYTES]),
                moods: NEUTRAL,
            },
            VectorSpec {
                name: "point_min",
                genome: point_count_genome(b"cortexkit-avatar/vectors/point-min/v1", 0),
                moods: NEUTRAL,
            },
            VectorSpec {
                name: "point_max",
                genome: point_count_genome(b"cortexkit-avatar/vectors/point-max/v1", 12),
                moods: NEUTRAL,
            },
            VectorSpec {
                name: "easing_edges",
                genome: Genome::from_bytes(easing_edges),
                moods: NEUTRAL,
            },
            VectorSpec {
                name: "pattern_0",
                genome: pattern_genome(0),
                moods: NEUTRAL,
            },
            VectorSpec {
                name: "pattern_1",
                genome: pattern_genome(1),
                moods: NEUTRAL,
            },
            VectorSpec {
                name: "pattern_2",
                genome: pattern_genome(2),
                moods: NEUTRAL,
            },
            VectorSpec {
                name: "pattern_3",
                genome: pattern_genome(3),
                moods: ALL_MOODS,
            },
            VectorSpec {
                name: "pattern_4",
                genome: pattern_genome(4),
                moods: NEUTRAL,
            },
            VectorSpec {
                name: "pattern_5",
                genome: pattern_genome(5),
                moods: NEUTRAL,
            },
            seat_vector("seat_ava", "pj-golden-ava", ALL_MOODS),
            seat_vector("seat_subc", "pj-golden-subc", NEUTRAL),
            seat_vector("seat_alf", "pj-golden-alf", NEUTRAL),
            seat_vector("seat_engram", "pj-golden-engram", NEUTRAL),
            seat_vector("seat_ckios", "pj-golden-ckios", NEUTRAL),
            seat_vector("seat_fusi", "pj-golden-fusi", NEUTRAL),
            VectorSpec {
                name: "sliver",
                genome: Genome::from_bytes(sliver),
                moods: NEUTRAL,
            },
            // Coverage vectors, chosen by measurement rather than taste.
            // `cargo run -p cortexkit-avatar-genome --example fleet_coverage` decoded
            // the fleet's 30 stored genomes against this corpus and found
            // nine decode-selector values live agents reach that no vector
            // exercised: body point counts 5, 7, 9 and 10, lash counts 1,
            // 6, 7 and 11, and mouth family 3 — a dense dispatch arm
            // implemented in three languages and byte-compared in none.
            // The seeds are a greedy minimum cover over that gap set, so
            // four vectors close all nine. They are derived from documented
            // strings rather than copied from registry rows, because a
            // stored genome can be re-rolled out from under the corpus.
            seat_vector("cover_point_count_5", "corpus-cover-1", ALL_MOODS),
            seat_vector("cover_point_count_9", "corpus-cover-56", NEUTRAL),
            seat_vector("cover_point_count_10", "corpus-cover-166", NEUTRAL),
            seat_vector("cover_point_count_7", "corpus-cover-185", NEUTRAL),
        ]
    }

    fn point_count_genome(label: &[u8], count_bits: u64) -> Genome {
        let mut bytes = xof_bytes(label);
        set_bits(&mut bytes, 0, 4, count_bits);
        Genome::from_bytes(bytes)
    }

    fn pattern_genome(class: u8) -> Genome {
        let label = format!("cortexkit-avatar/vectors/pattern-{class}/v1");
        let mut bytes = xof_bytes(label.as_bytes());
        set_bits(&mut bytes, 1_088, 8, u64::from(class));
        Genome::from_bytes(bytes)
    }

    fn seat_vector(name: &'static str, project_id: &str, moods: &'static [Mood]) -> VectorSpec {
        VectorSpec {
            name,
            genome: derive_genome(&SeatIdentity::Registered {
                project_id: project_id.to_owned(),
            }),
            moods,
        }
    }

    fn xof_bytes(label: &[u8]) -> [u8; GENOME_BYTES] {
        let mut bytes = [0; GENOME_BYTES];
        let mut hasher = blake3::Hasher::new();
        hasher.update(label);
        hasher.finalize_xof().fill(&mut bytes);
        bytes
    }

    fn set_bits(bytes: &mut [u8; GENOME_BYTES], offset: usize, width: usize, value: u64) {
        assert!(width <= 64, "test field is wider than u64");
        assert!(
            offset + width <= bytes.len() * 8,
            "test field is outside the genome"
        );
        assert!(
            width == 64 || value < (1_u64 << width),
            "test field value is too wide"
        );
        for bit_index in 0..width {
            let genome_bit = offset + bit_index;
            let byte_index = genome_bit / 8;
            let bit_in_byte = 7 - genome_bit % 8;
            let bit = (value >> (width - bit_index - 1)) & 1;
            bytes[byte_index] =
                (bytes[byte_index] & !(1 << bit_in_byte)) | ((bit as u8) << bit_in_byte);
        }
    }

    fn render_params(mood: Mood) -> RenderParams {
        RenderParams {
            mood,
            growth: Q32::ONE,
            canvas: 10_000,
        }
    }

    fn mood_name(mood: Mood) -> &'static str {
        match mood {
            Mood::Neutral => "neutral",
            Mood::Working => "working",
            Mood::Happy => "happy",
            Mood::Waiting => "waiting",
            Mood::Blocked => "blocked",
            Mood::Sad => "sad",
            Mood::Sleepy => "sleepy",
        }
    }

    fn corpus_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("vectors")
    }

    fn manifest_path() -> PathBuf {
        corpus_root().join("manifest.json")
    }

    fn dna_path(name: &str) -> PathBuf {
        corpus_root().join("dna").join(format!("{name}.hex"))
    }

    fn phenotype_path(name: &str) -> PathBuf {
        corpus_root().join("phenotype").join(format!("{name}.json"))
    }

    fn tree_path(name: &str, mood: Mood) -> PathBuf {
        corpus_root()
            .join("tree")
            .join(format!("{name}__{}.json", mood_name(mood)))
    }

    fn svg_path(name: &str, mood: Mood) -> PathBuf {
        corpus_root()
            .join("svg")
            .join(format!("{name}__{}.svg", mood_name(mood)))
    }

    fn tree_fixture_file_names() -> BTreeSet<String> {
        let mut names = WILD_TREE_FIXTURES
            .iter()
            .map(|(name, _, _)| (*name).to_owned())
            .collect::<BTreeSet<_>>();
        for vector in vector_specs() {
            for mood in vector.moods {
                names.insert(format!("{}__{}.json", vector.name, mood_name(*mood)));
            }
        }
        names
    }

    fn read_tree_fixture(name: &str) -> avatar_svg::RenderTree {
        let path = corpus_root().join("tree").join(name);
        let bytes =
            fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        crate::tree_fixture::decode_tree_fixture(&bytes)
            .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
    }

    fn tree_svg_fixture_name(tree_fixture_name: &str) -> String {
        format!(
            "{}.svg",
            tree_fixture_name
                .strip_suffix(".json")
                .expect("tree fixture name must end in .json")
        )
    }

    fn tree_svg_fixture_path(tree_fixture_name: &str) -> PathBuf {
        corpus_root()
            .join("tree-svg")
            .join(tree_svg_fixture_name(tree_fixture_name))
    }

    fn update_golden_enabled() -> bool {
        std::env::var_os("UPDATE_GOLDEN").is_some()
    }

    fn producer_sha(root: &Path) -> String {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(root.parent().expect("vectors root has repository parent"))
            .output()
            .expect("run git rev-parse HEAD while minting vectors");
        assert!(
            output.status.success(),
            "git rev-parse HEAD failed while minting vectors: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let sha = String::from_utf8(output.stdout)
            .expect("git rev-parse HEAD emitted UTF-8")
            .trim()
            .to_owned();
        assert_valid_git_sha(&sha);
        sha
    }

    fn manifest_bytes(producer_sha: &str) -> Vec<u8> {
        let mut manifest = format!(
            "{{\"corpus_revision\":{CORPUS_REVISION},\"genome_version\":{GENOME_VERSION},\"producer_sha\":\"{producer_sha}\",\"vectors\":["
        );
        for (index, vector) in vector_specs().iter().enumerate() {
            if index != 0 {
                manifest.push(',');
            }
            manifest.push_str("{\"moods\":[");
            for (mood_index, mood) in vector.moods.iter().enumerate() {
                if mood_index != 0 {
                    manifest.push(',');
                }
                manifest.push('"');
                manifest.push_str(mood_name(*mood));
                manifest.push('"');
            }
            manifest.push_str("],\"name\":\"");
            manifest.push_str(vector.name);
            manifest.push_str("\"}");
        }
        manifest.push_str("]}");
        manifest.into_bytes()
    }

    fn read_manifest() -> Manifest {
        let bytes = fs::read(manifest_path()).expect("read vectors/manifest.json");
        serde_json::from_slice(&bytes).expect("parse vectors/manifest.json")
    }

    fn assert_valid_git_sha(sha: &str) {
        assert_eq!(sha.len(), 40, "producer_sha must be a 40-character git SHA");
        assert!(
            sha.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "producer_sha must contain only hexadecimal digits"
        );
    }

    fn assert_current(path: &Path, expected: &[u8]) {
        let actual =
            fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        if expected == actual {
            return;
        }
        let offset = expected
            .iter()
            .zip(&actual)
            .position(|(expected, actual)| expected != actual)
            .unwrap_or(expected.len().min(actual.len()));
        panic!(
            "golden corpus drift in {}: first differing byte offset {offset} (expected {}, found {}); if the change to the renderer was intended, regenerate with UPDATE_GOLDEN=1 cargo test -p avatar-vectors tests::mint -- --ignored and LOOK at vectors/contact-sheet.html before committing. Changing what a shipped row renders to is a version event, and versions are added rather than edited: see docs/design/012-version-and-type-procedure.md",
            corpus_relative_path(path),
            described_byte(expected.get(offset)),
            described_byte(actual.get(offset)),
        );
    }

    fn corpus_relative_path(path: &Path) -> String {
        path.strip_prefix(
            corpus_root()
                .parent()
                .expect("vectors root has repository parent"),
        )
        .unwrap_or(path)
        .display()
        .to_string()
    }

    fn described_byte(byte: Option<&u8>) -> String {
        match byte {
            Some(byte) => format!("0x{byte:02x}"),
            None => "end of file".to_owned(),
        }
    }

    fn write_corpus_file(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
    }

    fn assert_directory_entry_names(relative_directory: &str, expected: &BTreeSet<String>) {
        let path = corpus_root()
            .parent()
            .expect("vectors root has repository parent")
            .join(relative_directory);
        let actual = entry_names(&path);
        assert_eq!(
            actual, *expected,
            "{relative_directory} does not contain exactly the committed corpus entries"
        );
    }

    fn assert_file_names(relative_directory: &str, expected: &BTreeSet<String>) {
        let path = corpus_root().join(relative_directory);
        let actual = entry_names(&path);
        assert_eq!(
            actual, *expected,
            "vectors/{relative_directory} does not contain exactly the files named by the manifest"
        );
        for name in expected {
            assert!(
                path.join(name).is_file(),
                "vectors/{relative_directory}/{name} is not a file"
            );
        }
    }

    fn entry_names(path: &Path) -> BTreeSet<String> {
        fs::read_dir(path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
            .map(|entry| {
                entry
                    .unwrap_or_else(|error| panic!("read entry in {}: {error}", path.display()))
                    .file_name()
                    .into_string()
                    .unwrap_or_else(|name| {
                        panic!(
                            "non-UTF-8 entry in {}: {:?}",
                            path.display(),
                            OsStr::new(&name)
                        )
                    })
            })
            .collect()
    }
}
