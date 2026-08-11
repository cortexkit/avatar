//! Throwaway visual smoke check: derive genomes for fleet seat names,
//! decode, render complete avatars, and write SVGs for eyeballing.
use avatar_genome::{decode, derive_genome, SeatIdentity};
use avatar_phenotype::Q32;
use avatar_svg::{render_svg, Mood, RenderParams};

fn main() {
    for seat in [
        "AVA", "SUBC", "ALF", "ENGRAM", "CKIOS", "BROCA", "FUSI", "MC",
    ] {
        let genome = derive_genome(&SeatIdentity::Registered {
            seat_name: seat.to_owned(),
            project_id: format!("pj-sample-{seat}"),
        });
        let phenotype = decode(&genome);
        let svg = render_svg(
            &phenotype,
            &RenderParams {
                mood: Mood::Neutral,
                growth: Q32::ONE,
                canvas: 10_000,
            },
        )
        .expect("render");
        std::fs::write(format!("/tmp/ava-samples/{seat}.svg"), svg).expect("write");
        println!(
            "{seat}: points={} pattern={}",
            phenotype.body.point_count, phenotype.pattern.r#class
        );
    }
}
