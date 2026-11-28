//! A real frame on a GPU. Worker sandboxes and build servers have no GPU
//! adapter, so this test is ignored there; run it on a machine with one:
//!
//! ```text
//! cargo test -p cortexkit-avatar-3d-wgpu --test render gpu -- --ignored
//! ```

use avatar_3d::motion::Actor;
use avatar_3d::robot::{
    build, build_with, face_set, BuildOptions, MoodSet, Robot, TextureContent, TEXTURE_PX,
};
use avatar_3d_wgpu::layers::{layer_bytes, Residency};
use avatar_3d_wgpu::view::View;
use avatar_3d_wgpu::{GpuRobot, Image, Item, Renderer, Scene};
use avatar_small_robots::{Mood, Rgb, GENOME_LEN};

use crate::genomes;

#[test]
#[ignore = "needs a GPU adapter"]
fn a_render_returns_rgba8_of_the_requested_size_with_the_robot_on_it() {
    let renderer = Renderer::new().expect("a GPU adapter and device");
    let robot = build(&genomes::corpus()[0]);
    let gpu = renderer.upload(&robot);
    let pose = Actor::new(&robot, Mood::Neutral).evaluate(0.0);
    let (width, height) = (200, 150);
    let background = Rgb {
        r: 0xee,
        g: 0xf0,
        b: 0xf3,
    };
    let image = renderer
        .render(&Scene {
            view: View::from(&robot.camera()).with_aspect(f64::from(width) / f64::from(height)),
            width,
            height,
            background,
            items: &[Item {
                robot: &robot,
                gpu: &gpu,
                pose: &pose,
                offset: [0.0; 3],
            }],
        })
        .expect("the frame reads back");
    assert_eq!((image.width, image.height), (width, height));
    assert_eq!(image.pixels.len(), (width * height * 4) as usize);
    let bg = [background.r, background.g, background.b];
    // The clear colour goes through the target's sRGB encoding, which may
    // round either way.
    let near_bg = |p: &[u8]| p.iter().zip(bg).all(|(a, b)| a.abs_diff(b) <= 1);
    assert!(near_bg(&image.pixels[..3]) && image.pixels[3] == 255);
    let robot_pixels = image
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| !near_bg(&p[..3]))
        .count();
    let total = (width * height) as usize;
    assert!(
        robot_pixels > total / 20 && robot_pixels < total * 9 / 10,
        "{robot_pixels} of {total} pixels differ from the background"
    );
}

#[test]
#[ignore = "needs a GPU adapter"]
fn every_drawn_frame_is_timed_on_the_gpu_when_the_device_has_timestamps() {
    let renderer = Renderer::new().expect("a GPU adapter and device");
    let robot = build(&genomes::corpus()[0]);
    let gpu = renderer.upload(&robot);
    renderer.finish_uploads().expect("the upload finishes");
    let actor = Actor::new(&robot, Mood::Neutral);
    let background = Rgb {
        r: 0xee,
        g: 0xf0,
        b: 0xf3,
    };
    for frame in 0..8 {
        let pose = actor.evaluate(f64::from(frame) / 30.0);
        let scene = Scene {
            view: View::from(&robot.camera()),
            width: 288,
            height: 288,
            background,
            items: &[Item {
                robot: &robot,
                gpu: &gpu,
                pose: &pose,
                offset: [0.0; 3],
            }],
        };
        let time = renderer.draw(&scene).wait().expect("the frame finishes");
        if renderer.timestamps() {
            // A time read before the GPU had written the frame's end would
            // come back missing, or end before it started.
            let t = time.unwrap_or_else(|| panic!("frame {frame} was not timed"));
            assert!(t > 0.0 && t < 0.25, "frame {frame} took {t} s on the GPU");
        } else {
            assert_eq!(time, None, "frame {frame} without timestamp queries");
        }
    }
}

/// The background of the layer tests' frames.
const BACKGROUND: Rgb = Rgb {
    r: 0x20,
    g: 0x24,
    b: 0x30,
};

/// A robot in `mood` at rest, drawn alone at 288 × 288 px.
fn draw(renderer: &Renderer, view: View, robot: &Robot, gpu: &GpuRobot, mood: Mood) -> Image {
    let pose = Actor::new(robot, mood).evaluate(0.0);
    renderer
        .render(&Scene {
            view,
            width: 288,
            height: 288,
            background: BACKGROUND,
            items: &[Item {
                robot,
                gpu,
                pose: &pose,
                offset: [0.0; 3],
            }],
        })
        .expect("the frame reads back")
}

/// The largest difference of any channel between two images of one size.
fn worst(a: &Image, b: &Image) -> u8 {
    assert_eq!((a.width, a.height), (b.width, b.height));
    a.pixels
        .iter()
        .zip(&b.pixels)
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0)
}

/// The indices of a robot's textures whose content passes `want`.
fn textures(robot: &Robot, want: impl Fn(TextureContent) -> bool) -> Vec<usize> {
    robot
        .textures
        .iter()
        .enumerate()
        .filter(|(_, t)| want(t.binding.content))
        .map(|(b, _)| b)
        .collect()
}

/// The face and stalk-eye textures of one mood, open and blinking.
fn face_textures(robot: &Robot, mood: Mood) -> Vec<usize> {
    textures(robot, |c| match c {
        TextureContent::Face { mood: m, .. } | TextureContent::StalkEye { mood: m, .. } => {
            m == mood
        }
        _ => false,
    })
}

/// The marks and chest textures, which every mood shows.
fn shared_textures(robot: &Robot) -> Vec<usize> {
    textures(robot, |c| {
        matches!(c, TextureContent::Marks | TextureContent::Chest)
    })
}

/// The first corpus genome whose robot has marks.
fn marked_genome() -> Box<[u8; GENOME_LEN]> {
    genomes::corpus()
        .into_iter()
        .find(|g| {
            build_with(g, BuildOptions::untextured())
                .textures
                .iter()
                .any(|t| t.binding.content == TextureContent::Marks)
        })
        .expect("a corpus robot has marks")
}

#[test]
#[ignore = "needs a GPU adapter"]
fn compacted_layers_draw_as_every_layer_resident_does() {
    let renderer = Renderer::new().expect("a GPU adapter and device");
    let genome = marked_genome();
    let robot = build(&genome);
    let view = View::from(&robot.camera());
    let mood = Mood::Sad;
    let all = renderer.upload(&robot);
    let reference = draw(&renderer, view, &robot, &all, mood);

    // Every texture has pixels, so the consuming upload makes them all
    // resident at their own layers, and frees the robot's copies.
    let mut owned = robot.clone();
    let mut gpu = renderer.upload_consuming(&mut owned);
    assert!(owned.textures.iter().all(|t| t.pixels.is_none()));
    assert_eq!(
        gpu.residency(),
        &Residency::every(robot.textures.len(), TEXTURE_PX)
    );
    assert_eq!(gpu.texture_bytes, all.texture_bytes);

    // Keeping one mood's faces with the marks and chest copies them on the
    // GPU into fewer layers, each below its own index.
    let mut keep = face_textures(&robot, mood);
    let marks = textures(&robot, |c| c == TextureContent::Marks);
    keep.extend(shared_textures(&robot));
    let before = gpu.texture_bytes;
    renderer.restock(&mut gpu, TEXTURE_PX, &keep, Vec::new());
    for &b in &keep {
        let layer = gpu.residency().layer(b);
        assert!(
            layer.is_some_and(|l| l < b as u32),
            "texture {b} is kept at layer {layer:?}"
        );
    }
    assert!(!marks.is_empty() && marks.iter().all(|m| keep.contains(m)));
    assert_eq!(gpu.residency().layer_count() as usize, keep.len());
    assert_eq!(
        gpu.texture_bytes,
        keep.len() as u64 * layer_bytes(TEXTURE_PX)
    );
    assert_eq!(
        renderer.retired_bytes(),
        before,
        "the old layers are retired"
    );

    let compacted = draw(&renderer, view, &owned, &gpu, mood);
    let diff = worst(&reference, &compacted);
    assert!(diff <= 1, "compacted, a channel differs by {diff}");

    // Once the GPU has finished the frames that sampled it, the old array
    // texture is freed.
    renderer.finish_uploads().expect("the GPU finishes");
    assert_eq!(renderer.retired_bytes(), 0);
}

#[test]
#[ignore = "needs a GPU adapter"]
fn fresh_face_sets_fill_compacted_layers_at_either_edge() {
    let renderer = Renderer::new().expect("a GPU adapter and device");
    let genome = marked_genome();
    let reference = build(&genome);
    let view = View::from(&reference.camera());
    let reference_gpu = renderer.upload(&reference);
    let mood = Mood::Sad;

    // Built with the neutral faces only, as a robot first shown neutral is.
    let neutral = MoodSet::NONE.with(Mood::Neutral);
    let mut robot = build_with(&genome, BuildOptions::default().with_moods(neutral));
    let mut gpu = renderer.upload_consuming(&mut robot);
    let shared = shared_textures(&robot);
    let neutral_faces = face_textures(&robot, Mood::Neutral);
    assert_eq!(
        gpu.residency().layer_count() as usize,
        shared.len() + neutral_faces.len(),
        "only the textures with pixels are resident"
    );

    // The sad faces come in fresh in place of the neutral ones; the marks
    // and chest are copied.
    let sad_faces = face_textures(&robot, mood);
    renderer.restock(
        &mut gpu,
        TEXTURE_PX,
        &shared,
        face_set(&genome, mood, TEXTURE_PX),
    );
    assert!(neutral_faces
        .iter()
        .all(|&b| gpu.residency().layer(b).is_none()));
    assert!(sad_faces
        .iter()
        .all(|&b| gpu.residency().layer(b).is_some_and(|l| l < b as u32)));
    let diff = worst(
        &draw(&renderer, view, &reference, &reference_gpu, mood),
        &draw(&renderer, view, &robot, &gpu, mood),
    );
    assert!(diff <= 1, "a fresh face set: a channel differs by {diff}");

    // At half the edge nothing is kept: the marks and chest come from a
    // build at that edge, with the face set drawn at it.
    let half = TEXTURE_PX / 2;
    let small = build_with(&genome, BuildOptions::default().with_texture_px(half));
    let small_gpu = renderer.upload(&small);
    let mut fresh = face_set(&genome, mood, half);
    for &b in &shared {
        let pixels = small.textures[b].pixels.clone();
        fresh.push((b, pixels.expect("marks and chest are drawn")));
    }
    renderer.restock(&mut gpu, half, &[], fresh);
    assert_eq!(gpu.residency().px(), half);
    assert_eq!(
        gpu.texture_bytes,
        (shared.len() + sad_faces.len()) as u64 * layer_bytes(half)
    );
    let diff = worst(
        &draw(&renderer, view, &small, &small_gpu, mood),
        &draw(&renderer, view, &robot, &gpu, mood),
    );
    assert!(diff <= 1, "at {half} px: a channel differs by {diff}");

    renderer.finish_uploads().expect("the GPU finishes");
    assert_eq!(renderer.retired_bytes(), 0);
}

#[test]
#[ignore = "needs a GPU adapter"]
fn a_128_px_build_of_two_moods_uploads_at_its_edge_with_layers_left_to_fill() {
    let renderer = Renderer::new().expect("a GPU adapter and device");
    let genome = &genomes::corpus()[0];
    let options = BuildOptions::default().with_texture_px(128);
    let two = build_with(
        genome,
        options.with_moods([Mood::Neutral, Mood::Happy].into_iter().collect()),
    );
    let every = build_with(genome, options);
    let view = View::from(&every.camera());
    let n = two.textures.len();

    let mut gpu = renderer.upload(&two);
    assert_eq!(gpu.residency(), &Residency::every(n, 128));
    assert_eq!(gpu.texture_bytes, n as u64 * layer_bytes(128));
    let every_gpu = renderer.upload(&every);
    assert_eq!(gpu.texture_bytes, every_gpu.texture_bytes);

    // A mood of the build draws as a build of every mood does.
    let diff = worst(
        &draw(&renderer, view, &two, &gpu, Mood::Happy),
        &draw(&renderer, view, &every, &every_gpu, Mood::Happy),
    );
    assert!(diff <= 1, "happy: a channel differs by {diff}");

    // Another mood's layers are empty, so its face is missing...
    let sad = draw(&renderer, view, &every, &every_gpu, Mood::Sad);
    let diff = worst(&draw(&renderer, view, &two, &gpu, Mood::Sad), &sad);
    assert!(diff > 1, "an empty face layer draws as the drawn face");

    // ...until its face set fills them.
    let fill = face_set(genome, Mood::Sad, 128);
    let keep: Vec<usize> = (0..n)
        .filter(|b| fill.iter().all(|(f, _)| f != b))
        .collect();
    renderer.restock(&mut gpu, 128, &keep, fill);
    assert_eq!(gpu.residency(), &Residency::every(n, 128));
    let diff = worst(&draw(&renderer, view, &two, &gpu, Mood::Sad), &sad);
    assert!(diff <= 1, "sad, filled: a channel differs by {diff}");
}
