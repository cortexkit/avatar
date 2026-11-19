//! Committed cross-port conformance vectors for the avatar pipeline.

#[cfg(test)]
pub(crate) mod test_util {
    /// Checks the SVG tag stack without adding an XML parser dependency to this
    /// test-only crate. The renderer emits simple XML, so tag balance and quoted
    /// attributes are sufficient smoke coverage for the committed corpus.
    pub(crate) fn assert_well_formed_svg(label: &str, svg: &str) {
        let mut stack: Vec<String> = Vec::new();
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
            let tag_end = svg[index..].find('>').expect("unterminated tag");
            let tag = &svg[index + 1..index + tag_end];
            if let Some(name) = tag.strip_prefix('/') {
                let expected = stack.pop().expect("closing tag without an open tag");
                assert_eq!(name.trim(), expected, "{label}: mismatched closing tag");
            } else {
                let name: String = tag
                    .chars()
                    .take_while(|character| {
                        character.is_ascii_alphanumeric() || matches!(character, ':' | '-')
                    })
                    .collect();
                assert!(!name.is_empty(), "{label}: empty tag name at byte {index}");
                let attributes = &tag[name.len()..];
                let quotes = attributes.bytes().filter(|byte| *byte == b'"').count();
                assert_eq!(
                    quotes % 2,
                    0,
                    "{label}: unbalanced attribute quotes in <{name}>"
                );
                if !attributes.trim_end().ends_with('/') {
                    stack.push(name);
                }
            }
            index += tag_end + 1;
        }
        assert!(stack.is_empty(), "{label}: unclosed tags: {stack:?}");
    }
}

#[cfg(test)]
mod tree_fixture;

#[cfg(test)]
mod tests {
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
    // CLASSIC_VERSION in avatar_svg::dispatch identifies the algorithm used to
    // render stored genomes. Adding fixture cases must not change that version
    // or the appearance rendered for existing identities. This counter changes
    // only when the classic corpus is re-minted.
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
    const TREE_SVG_BOUNDARY_SIZE: usize = 1_017_856;
    const TREE_SVG_BOUNDARY_HASH: &str =
        "84503da4b59fcc6e5c14a0159a39c90e685403d167892b771105fed930da8dd3";

    /// Pins for the identity-derived wild corpus: neutral for each genome,
    /// plus all seven moods for one genome. Their gradient paint, masked
    /// gradients and path fill rules must survive decoding in Swift and
    /// TypeScript, even though classic trees do not exercise those primitives.
    const WILD_TREE_FIXTURES: &[(&str, usize, &str)] = &[
        (
            "wild-port-0005__neutral.json",
            220985,
            "1a09fff92495826f337d7b2e45739bccfc666c23ca3b5c969c9144d6463bee89",
        ),
        (
            "wild-port-0005__working.json",
            221093,
            "652cfc849dfbf3541a295190817e030c16a92d5b89eae5befc1ca9a7e223b1c9",
        ),
        (
            "wild-port-0005__happy.json",
            202259,
            "6560a27f7fbc0e19cffa395c736ff6d887a7afaa9fa01afbf237a52cd19452e0",
        ),
        (
            "wild-port-0005__waiting.json",
            220569,
            "e6ff827fdd1716cc769ecef84910e8e3463410354cc09ef82b563d9186991d2a",
        ),
        (
            "wild-port-0005__blocked.json",
            220627,
            "115d563302d6151b398108b572aa858f0feabf1e52415f0de415f90167be4b0d",
        ),
        (
            "wild-port-0005__sad.json",
            220680,
            "b69ce14c9d13051d5e69846b3c7a130072d389f41ed11e0179855aa31745d46b",
        ),
        (
            "wild-port-0005__sleepy.json",
            258660,
            "9420c817e3cb0b9530adb459621ee79699bc8bc7860b2ebb02f92b78ff32d3d8",
        ),
        (
            "wild-port-0031__neutral.json",
            87082,
            "cad1ea55cfdd5444002060140ba6927e09da9a43177150c1908233086764f230",
        ),
        (
            "wild-port-0009__neutral.json",
            305327,
            "72d661c63380e9f15ada7ba2a9b9f19699d124e7dda1d84b44dccee93592960a",
        ),
        (
            "wild-port-0022__neutral.json",
            126991,
            "9b086c1b6bacd798503fb5c9f9731852226b303dd42020df0e5a7a4a2e581b16",
        ),
        (
            "wild-port-0077__neutral.json",
            200572,
            "adf06235a738729d5507f59df60e132da19b4ad96102fa6198bba7995751f7d7",
        ),
        (
            "wild-port-0043__neutral.json",
            127148,
            "76dd0d8b69744ae6bc0b5f14506782bec08f8a40b462842062cc98652bb34827",
        ),
        (
            "wild-port-0016__neutral.json",
            165029,
            "b246d54be64358970112fed6922357f0e59c693915fdbb8d99493e5ebe3b0809",
        ),
        (
            "wild-port-0021__neutral.json",
            263844,
            "997e9c5660d24f5608e9a68f6e6c1d2a018330ddca262bb16f2fab134e28c02a",
        ),
        (
            "wild-port-0000__neutral.json",
            185111,
            "093db383c12dd22515e92e420a5aea55a6660533bbd6e6d748089b3af4de65aa",
        ),
        (
            "wild-port-0001__neutral.json",
            95133,
            "01231d31bc69e4dfe2866137813c944d3f381f82244cbf270a833f67d20a8ebb",
        ),
        (
            "wild-port-0002__neutral.json",
            345337,
            "dd4ca42261634625d82919db1aa76b5f475d932c540028f5d621397f4afc0534",
        ),
        (
            "wild-port-0007__neutral.json",
            71495,
            "01644b9bc037b251abda6155a3d8c64898f91894522dbbab44a74ee98bfbf8f5",
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
            "manifest.json".to_owned(),
            "phenotype".to_owned(),
            "svg".to_owned(),
            "tree".to_owned(),
            "tree-svg".to_owned(),
            "wild".to_owned(),
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
            // `cargo run -p avatar-genome --example fleet_coverage` decoded
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
