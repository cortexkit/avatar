//! Developer-only visual smoke check for experimental right-side asymmetry and layered-body previews.
use std::path::Path;

use avatar_genome::{decode, derive_genome, SeatIdentity};
use avatar_phenotype::Q32;
use avatar_svg::{
    experimental::{
        build_render_tree_experimental, serialize_svg_experimental, ExperimentalOptions,
    },
    render_svg, Mood, RenderParams,
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

        std::fs::write(
            output.join(format!("{seat}-stable.svg")),
            render_svg(&phenotype, &params).expect("render stable preview"),
        )
        .expect("write stable preview");
        // Baseline through the SAME preview projection as the variants: the
        // projection skips lighting, so comparing a variant against the stable
        // serializer's lit output would show shading differences as if they
        // were geometry changes.
        std::fs::write(
            output.join(format!("{seat}-base.svg")),
            render_preview(
                &phenotype,
                &params,
                ExperimentalOptions {
                    asymmetry_strength: Q32(0),
                    layers: false,
                },
            ),
        )
        .expect("write baseline preview");
        std::fs::write(
            output.join(format!("{seat}-asym.svg")),
            render_preview(
                &phenotype,
                &params,
                ExperimentalOptions {
                    asymmetry_strength: Q32::ONE,
                    layers: false,
                },
            ),
        )
        .expect("write asymmetry preview");
        std::fs::write(
            output.join(format!("{seat}-layers.svg")),
            render_preview(
                &phenotype,
                &params,
                ExperimentalOptions {
                    asymmetry_strength: Q32(0),
                    layers: true,
                },
            ),
        )
        .expect("write layers preview");
        let both = build_render_tree_experimental(
            &phenotype,
            &params,
            &ExperimentalOptions {
                asymmetry_strength: Q32::ONE,
                layers: true,
            },
        )
        .expect("build combined preview");
        let layer_count = both
            .primitives
            .iter()
            .filter(|primitive| primitive.label.starts_with("av-experimental-layer-"))
            .count();
        std::fs::write(
            output.join(format!("{seat}-both.svg")),
            serialize_svg_experimental(&both),
        )
        .expect("write combined preview");

        println!(
            "{seat}: points={} layers_drawn={layer_count}",
            phenotype.body.point_count
        );
    }
}

fn render_preview(
    phenotype: &avatar_phenotype::Phenotype,
    params: &RenderParams,
    options: ExperimentalOptions,
) -> Vec<u8> {
    let tree = build_render_tree_experimental(phenotype, params, &options).expect("build preview");
    serialize_svg_experimental(&tree)
}
