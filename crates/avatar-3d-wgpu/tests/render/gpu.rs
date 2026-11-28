//! A real frame on a GPU. Worker sandboxes and build servers have no GPU
//! adapter, so this test is ignored there; run it on a machine with one:
//!
//! ```text
//! cargo test -p cortexkit-avatar-3d-wgpu --test render gpu -- --ignored
//! ```

use avatar_3d::motion::Actor;
use avatar_3d::robot::build;
use avatar_3d_wgpu::view::View;
use avatar_3d_wgpu::{Item, Renderer, Scene};
use avatar_small_robots::{Mood, Rgb};

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
