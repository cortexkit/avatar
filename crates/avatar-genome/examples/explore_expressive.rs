//! Developer-only visual smoke check for experimental right-side asymmetry and layered-body previews.
//!
//! Spike-2 grid: an asymmetry strength sweep (the spike-1 strength of 1.0 was
//! visually negligible) and both layer paint schemes, all through the unlit
//! preview projection so variants compare like-for-like.
use std::path::Path;

use avatar_genome::{decode, derive_genome, SeatIdentity};
use avatar_phenotype::{Phenotype, Q32};
use avatar_svg::{
    experimental::{
        build_render_tree_experimental, serialize_svg_experimental, ExperimentalOptions, LayerPaint,
    },
    Mood, RenderParams,
};

const SEATS: [&str; 6] = ["AVA", "SUBC", "ALF", "ENGRAM", "CKIOS", "BROCA"];

fn main() {
    let output = Path::new("/tmp/ava-explore");
    std::fs::create_dir_all(output).expect("create preview directory");

    for seat in SEATS {
        let genome = derive_genome(&SeatIdentity::Registered {
            project_id: format!("pj-expressive-{seat}"),
        });
        let phenotype = decode(&genome);
        let params = RenderParams {
            mood: Mood::Neutral,
            growth: Q32::ONE,
            canvas: 10_000,
        };

        let mut variants: Vec<(String, ExperimentalOptions)> = vec![(
            "base".to_owned(),
            ExperimentalOptions {
                asymmetry_strength: Q32(0),
                layers: false,
                layer_paint: LayerPaint::Harmonic,
            },
        )];
        // Strength N maps to N.0 in Q32.32: the sweep answers "where does
        // asymmetry become visible, and where does it stop being the same
        // creature".
        for strength in 1..=4i64 {
            variants.push((
                format!("asym{strength}"),
                ExperimentalOptions {
                    asymmetry_strength: Q32(strength * Q32::ONE.0),
                    layers: false,
                    layer_paint: LayerPaint::Harmonic,
                },
            ));
        }
        variants.push((
            "layers-harmonic".to_owned(),
            ExperimentalOptions {
                asymmetry_strength: Q32(0),
                layers: true,
                layer_paint: LayerPaint::Harmonic,
            },
        ));
        variants.push((
            "layers-ramp".to_owned(),
            ExperimentalOptions {
                asymmetry_strength: Q32(0),
                layers: true,
                layer_paint: LayerPaint::BodyRamp,
            },
        ));

        for (name, options) in &variants {
            std::fs::write(
                output.join(format!("{seat}-{name}.svg")),
                render_preview(&phenotype, &params, options),
            )
            .expect("write preview");
        }
        println!(
            "{seat}: points={} variants={}",
            phenotype.body.point_count,
            variants.len()
        );
    }
}

fn render_preview(
    phenotype: &Phenotype,
    params: &RenderParams,
    options: &ExperimentalOptions,
) -> Vec<u8> {
    let tree = build_render_tree_experimental(phenotype, params, options).expect("build preview");
    serialize_svg_experimental(&tree)
}
