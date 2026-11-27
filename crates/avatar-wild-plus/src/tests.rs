use super::*;

#[test]
fn every_mood_draws_with_one_ground_shadow() {
    let bytes = avatar_svg::wild::probe_genome(7);
    for mood in mood::MOODS {
        let tree = render_tree(&bytes, mood, None);
        let shadows = tree
            .primitives
            .iter()
            .filter(|p| p.role == Some(avatar_svg::Role::GroundShadow))
            .count();
        assert_eq!(shadows, 1);
    }
}
