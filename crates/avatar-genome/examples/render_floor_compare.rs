//! Throwaway evidence for the point-count floor decision: render the two
//! degenerate 2-point seat genomes as-is, then re-render the same genomes
//! with the count field forced to 4 (bits 0-3 set to 2, since 2+(2 mod 15)=4)
//! so only the floor differs while every other trait stays theirs.
use avatar_genome::{decode, derive_genome, Genome, SeatIdentity};
use avatar_phenotype::Q32;
use avatar_svg::{render_svg, Mood, RenderParams};

fn main() {
    for seat in ["basil", "fern"] {
        let genome = derive_genome(&SeatIdentity::Registered {
            project_id: format!("pj-sample-{seat}"),
        });
        let mut raised = *genome.as_bytes();
        raised[0] = (raised[0] & 0x0f) | 0x20; // count bits 0-3 := 2 -> point_count 4
        for (label, g) in [("floor2", genome), ("floor4", Genome::from_bytes(raised))] {
            let p = decode(&g);
            let svg = render_svg(
                &p,
                &RenderParams {
                    mood: Mood::Neutral,
                    growth: Q32::ONE,
                    canvas: 10_000,
                },
            )
            .expect("render");
            std::fs::write(format!("/tmp/ava-samples/{seat}-{label}.svg"), svg).expect("write");
            println!("{seat}-{label}: points={}", p.body.point_count);
        }
    }
}
