//! Throwaway visual smoke check: one seat rendered at every mood.
use avatar_genome::{decode, derive_genome, SeatIdentity};
use avatar_phenotype::Q32;
use avatar_svg::{render_svg, Mood, RenderParams};

fn main() {
    let genome = derive_genome(&SeatIdentity::Registered {
        project_id: "pj-sample-ALF".to_owned(),
    });
    let phenotype = decode(&genome);
    for (name, mood) in [
        ("neutral", Mood::Neutral),
        ("working", Mood::Working),
        ("happy", Mood::Happy),
        ("waiting", Mood::Waiting),
        ("blocked", Mood::Blocked),
        ("sad", Mood::Sad),
        ("sleepy", Mood::Sleepy),
    ] {
        let svg = render_svg(
            &phenotype,
            &RenderParams {
                mood,
                growth: Q32::ONE,
                canvas: 10_000,
            },
        )
        .expect("render");
        std::fs::write(format!("/tmp/ava-samples/mood-{name}.svg"), svg).expect("write");
    }
    println!("ok");
}
