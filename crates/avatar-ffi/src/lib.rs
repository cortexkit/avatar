//! Allocation-owning C boundary for the canonical Rust render-tree pipeline
//! and the finished images drawn from it.

use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

use avatar::{
    render_image, render_tree_for_bytes, serialize_tree_json, Alpha, DispatchError, GroundShadow,
    ImageOptions, Mood,
};

const OK: i32 = 0;
const UNKNOWN_TYPE: i32 = 1;
const UNKNOWN_VERSION: i32 = 2;
const GENOME_LENGTH: i32 = 3;
const UNKNOWN_MOOD: i32 = 4;
const INVALID_ARGUMENT: i32 = 5;
const RENDER_ERROR: i32 = 6;
const PANIC: i32 = 7;

fn boundary(f: impl FnOnce() -> i32) -> i32 {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(PANIC)
}

/// The mood a caller named, or `None` for a name the core does not know.
fn parse_mood(name: &CStr) -> Option<Mood> {
    Some(match name.to_bytes() {
        b"neutral" => Mood::Neutral,
        b"working" => Mood::Working,
        b"happy" => Mood::Happy,
        b"waiting" => Mood::Waiting,
        b"blocked" => Mood::Blocked,
        b"sad" => Mood::Sad,
        b"sleepy" => Mood::Sleepy,
        _ => return None,
    })
}

/// The status a dispatch refusal is reported with.
fn dispatch_status(error: &DispatchError) -> i32 {
    match error {
        DispatchError::UnsupportedType { .. } => UNKNOWN_TYPE,
        DispatchError::UnsupportedVersion { .. } => UNKNOWN_VERSION,
        DispatchError::GenomeLength { .. } => GENOME_LENGTH,
        // An edge length outside 1..=2,048 is a bad argument from the
        // caller, like an image flag other than 0 or 1. Only the image call
        // checks a size, so the tree call never returns it.
        DispatchError::ImageSize { .. } => INVALID_ARGUMENT,
        // Only the image call rasterizes, so the tree call never returns
        // Raster.
        DispatchError::Render(_) | DispatchError::Raster(_) => RENDER_ERROR,
    }
}

/// Renders through the versioned dispatch without retaining any caller memory.
///
/// # Safety
/// Non-null inputs must be readable for their declared lengths; strings must
/// be NUL-terminated. `out_json` must be writable and must not alias the inputs.
/// The returned allocation must be released with `avatar_string_free`.
#[no_mangle]
pub unsafe extern "C" fn avatar_render_tree_json(
    genome_ptr: *const u8,
    genome_len: usize,
    type_cstr: *const c_char,
    version: u32,
    mood_cstr: *const c_char,
    out_json: *mut *mut c_char,
) -> i32 {
    boundary(|| {
        if out_json.is_null() {
            return INVALID_ARGUMENT;
        }
        *out_json = ptr::null_mut();
        if genome_ptr.is_null() || type_cstr.is_null() || mood_cstr.is_null() {
            return INVALID_ARGUMENT;
        }
        // No length check here. Dispatch checks type, then version, then
        // the length THAT TYPE requires, and that order is what lets a
        // client draw nothing for a type it cannot depict while reporting a
        // malformed row. A fixed length here would refuse every genome of a
        // newer, differently sized type as corrupt.
        let Ok(avatar_type) = CStr::from_ptr(type_cstr).to_str() else {
            return UNKNOWN_TYPE;
        };
        let Some(mood) = parse_mood(CStr::from_ptr(mood_cstr)) else {
            return UNKNOWN_MOOD;
        };
        let bytes = std::slice::from_raw_parts(genome_ptr, genome_len);
        let tree = match render_tree_for_bytes(bytes, avatar_type, version, mood) {
            Ok(tree) => tree,
            Err(error) => return dispatch_status(&error),
        };
        let Ok(json) = CString::new(serialize_tree_json(&tree)) else {
            return RENDER_ERROR;
        };
        *out_json = json.into_raw();
        OK
    })
}

/// Releases a string allocated by this library; null is a no-op.
///
/// # Safety
/// `ptr` must be null or a still-live allocation returned by
/// `avatar_render_tree_json`, and must not be freed more than once.
#[no_mangle]
pub unsafe extern "C" fn avatar_string_free(ptr: *mut c_char) {
    // Reconstructing a valid CString cannot panic; keeping the boundary here
    // also prevents future cleanup changes from unwinding into a C caller.
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !ptr.is_null() {
            drop(CString::from_raw(ptr));
        }
    }));
}

/// Draws a stored avatar as a finished square RGBA image through the
/// versioned dispatch, without retaining any caller memory.
///
/// `drop_ground_shadow` and `straight_alpha` must each be 0 or 1. Any other
/// value, a null argument or a `size_px` outside 1..=2,048 returns
/// `INVALID_ARGUMENT`. The size is checked after the type, version and
/// genome length, as `avatar::render_image` checks it, so a stored avatar
/// whose type, version or genome length this build refuses is reported as
/// such whatever size was asked for.
///
/// # Safety
/// Non-null inputs must be readable for their declared lengths; strings must
/// be NUL-terminated. `out_pixels` and `out_len` must be writable and must
/// not alias each other or the inputs. The returned allocation must be
/// released with `avatar_image_free`, passing the length returned with it.
#[no_mangle]
pub unsafe extern "C" fn avatar_render_image(
    genome_ptr: *const u8,
    genome_len: usize,
    type_cstr: *const c_char,
    version: u32,
    mood_cstr: *const c_char,
    size_px: u32,
    drop_ground_shadow: u32,
    straight_alpha: u32,
    out_pixels: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    boundary(|| {
        // Clear every output the caller gave before anything can fail. The
        // outputs are written again only once the image exists, so every
        // failure, a contained panic included, leaves them null and 0.
        if !out_pixels.is_null() {
            *out_pixels = ptr::null_mut();
        }
        if !out_len.is_null() {
            *out_len = 0;
        }
        if out_pixels.is_null() || out_len.is_null() {
            return INVALID_ARGUMENT;
        }
        if genome_ptr.is_null() || type_cstr.is_null() || mood_cstr.is_null() {
            return INVALID_ARGUMENT;
        }
        let ground_shadow = match drop_ground_shadow {
            0 => GroundShadow::Keep,
            1 => GroundShadow::Drop,
            _ => return INVALID_ARGUMENT,
        };
        let alpha = match straight_alpha {
            0 => Alpha::Premultiplied,
            1 => Alpha::Straight,
            _ => return INVALID_ARGUMENT,
        };
        // As in the tree call, no length or size check happens here:
        // dispatch checks type, then version, then length, then size.
        let Ok(avatar_type) = CStr::from_ptr(type_cstr).to_str() else {
            return UNKNOWN_TYPE;
        };
        let Some(mood) = parse_mood(CStr::from_ptr(mood_cstr)) else {
            return UNKNOWN_MOOD;
        };
        let bytes = std::slice::from_raw_parts(genome_ptr, genome_len);
        let options = ImageOptions {
            ground_shadow,
            alpha,
        };
        let image = match render_image(bytes, avatar_type, version, mood, size_px, &options) {
            Ok(image) => image,
            Err(error) => return dispatch_status(&error),
        };
        // A boxed slice's allocation is exactly its length, so the pointer
        // and length handed out are all avatar_image_free needs to rebuild
        // it. Each call boxes a new image, so no later call can touch it.
        let pixels = image.pixels.into_boxed_slice();
        let len = pixels.len();
        *out_pixels = Box::into_raw(pixels).cast::<u8>();
        *out_len = len;
        OK
    })
}

/// Releases pixels returned by `avatar_render_image`; null is a no-op.
///
/// # Safety
/// `pixels` must be null, or a still-live allocation returned by
/// `avatar_render_image` with `len` exactly the length returned with it, and
/// must not be freed more than once.
#[no_mangle]
pub unsafe extern "C" fn avatar_image_free(pixels: *mut u8, len: usize) {
    // Rebuilding the boxed slice cannot panic; the boundary keeps any future
    // cleanup change from unwinding into a C caller.
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !pixels.is_null() {
            drop(Box::from_raw(ptr::slice_from_raw_parts_mut(pixels, len)));
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(bytes: &[u8], avatar_type: &str, version: u32, mood: &str) -> (i32, *mut c_char) {
        let avatar_type = CString::new(avatar_type).unwrap();
        let mood = CString::new(mood).unwrap();
        let mut output = ptr::dangling_mut();
        let status = unsafe {
            avatar_render_tree_json(
                bytes.as_ptr(),
                bytes.len(),
                avatar_type.as_ptr(),
                version,
                mood.as_ptr(),
                &mut output,
            )
        };
        (status, output)
    }

    #[test]
    fn success_returns_the_existing_canonical_json_for_every_mood() {
        let bytes = [0x5a; 1_024];
        for (name, mood) in [
            ("neutral", Mood::Neutral),
            ("working", Mood::Working),
            ("happy", Mood::Happy),
            ("waiting", Mood::Waiting),
            ("blocked", Mood::Blocked),
            ("sad", Mood::Sad),
            ("sleepy", Mood::Sleepy),
        ] {
            let (status, output) = render(&bytes, avatar::CLASSIC, 2, name);
            assert_eq!(status, OK);
            assert!(!output.is_null());
            let expected = serialize_tree_json(
                &render_tree_for_bytes(&bytes, avatar::CLASSIC, 2, mood).unwrap(),
            );
            unsafe {
                assert_eq!(CStr::from_ptr(output).to_str().unwrap(), expected);
                avatar_string_free(output);
            }
        }
    }

    #[test]
    fn errors_have_distinct_statuses_and_clear_the_output() {
        for (bytes, avatar_type, version, mood, expected) in [
            (&[0; 1_024][..], "unknown", 2, "neutral", UNKNOWN_TYPE),
            (
                &[0; 1_024][..],
                avatar::CLASSIC,
                999,
                "neutral",
                UNKNOWN_VERSION,
            ),
            (
                &[0; 1_023][..],
                avatar::CLASSIC,
                2,
                "neutral",
                GENOME_LENGTH,
            ),
            (&[0; 1_024][..], avatar::CLASSIC, 2, "unknown", UNKNOWN_MOOD),
        ] {
            let (status, output) = render(bytes, avatar_type, version, mood);
            assert_eq!(status, expected);
            assert!(output.is_null());
        }
    }

    #[test]
    fn an_unknown_type_is_reported_as_unknown_whatever_its_genome_length() {
        // A row of a type this build does not implement arrives with that
        // type's own genome size. It must read as "cannot depict" so the
        // client draws nothing, never as "malformed" so the client raises
        // a false corruption report for every such row.
        let newer_type_genome = [0x5a; 16_384];
        let (status, output) = render(&newer_type_genome, "creature.newer", 1, "neutral");
        assert_eq!(status, UNKNOWN_TYPE);
        assert!(output.is_null());

        // The same length under the KNOWN type is still a malformed row.
        let (status, output) = render(&newer_type_genome, avatar::CLASSIC, 2, "neutral");
        assert_eq!(status, GENOME_LENGTH);
        assert!(output.is_null());
    }

    #[test]
    fn null_arguments_are_rejected_and_null_free_is_safe() {
        let mut output = ptr::dangling_mut();
        unsafe {
            assert_eq!(
                avatar_render_tree_json(
                    ptr::null(),
                    1_024,
                    ptr::null(),
                    2,
                    ptr::null(),
                    &mut output
                ),
                INVALID_ARGUMENT
            );
            assert!(output.is_null());
            assert_eq!(
                avatar_render_tree_json(
                    ptr::null(),
                    0,
                    ptr::null(),
                    2,
                    ptr::null(),
                    ptr::null_mut()
                ),
                INVALID_ARGUMENT
            );
            avatar_string_free(ptr::null_mut());
        }
    }

    #[test]
    fn panics_are_contained_as_a_distinct_status() {
        assert_eq!(boundary(|| panic!("boundary probe")), PANIC);
    }

    #[test]
    fn concurrent_calls_own_independent_strings() {
        let expected: Vec<_> = (0..8)
            .map(|seed| {
                let (status, output) = render(&[seed; 1_024], avatar::CLASSIC, 2, "happy");
                assert_eq!(status, OK);
                assert!(!output.is_null());
                unsafe {
                    let bytes = CStr::from_ptr(output).to_bytes().to_vec();
                    avatar_string_free(output);
                    bytes
                }
            })
            .collect();
        let outputs = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|seed| {
                    scope.spawn(move || {
                        let (status, output) = render(&[seed; 1_024], avatar::CLASSIC, 2, "happy");
                        assert_eq!(status, OK);
                        assert!(!output.is_null());
                        // Transfer ownership across the join without copying or
                        // releasing the allocation returned by the FFI.
                        unsafe { CString::from_raw(output) }
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });
        for (index, (output, expected)) in outputs.iter().zip(&expected).enumerate() {
            assert_eq!(output.to_bytes(), expected);
            for other in &outputs[..index] {
                assert_ne!(output.as_ptr(), other.as_ptr());
            }
        }
        for output in outputs {
            unsafe { avatar_string_free(output.into_raw()) };
        }
    }

    const MOODS: [(&str, Mood); 7] = [
        ("neutral", Mood::Neutral),
        ("working", Mood::Working),
        ("happy", Mood::Happy),
        ("waiting", Mood::Waiting),
        ("blocked", Mood::Blocked),
        ("sad", Mood::Sad),
        ("sleepy", Mood::Sleepy),
    ];

    /// A length no successful image has, so a test can see that a failed
    /// call reset `out_len` to 0.
    const UNSET_LEN: usize = usize::MAX;

    /// Calls `avatar_render_image` with both outputs set to values a cleared
    /// output never holds, and returns the status and both outputs.
    fn render_image_raw(
        bytes: &[u8],
        avatar_type: &str,
        version: u32,
        mood: &str,
        size_px: u32,
        drop_ground_shadow: u32,
        straight_alpha: u32,
    ) -> (i32, *mut u8, usize) {
        let avatar_type = CString::new(avatar_type).unwrap();
        let mood = CString::new(mood).unwrap();
        let mut pixels = ptr::dangling_mut();
        let mut len = UNSET_LEN;
        let status = unsafe {
            avatar_render_image(
                bytes.as_ptr(),
                bytes.len(),
                avatar_type.as_ptr(),
                version,
                mood.as_ptr(),
                size_px,
                drop_ground_shadow,
                straight_alpha,
                &mut pixels,
                &mut len,
            )
        };
        (status, pixels, len)
    }

    /// Copies an image out of the core's allocation, then frees it.
    fn take_pixels(pixels: *mut u8, len: usize) -> Vec<u8> {
        assert!(!pixels.is_null());
        unsafe {
            let copy = std::slice::from_raw_parts(pixels, len).to_vec();
            avatar_image_free(pixels, len);
            copy
        }
    }

    fn first_difference(actual: &[u8], expected: &[u8]) -> Option<usize> {
        if actual.len() != expected.len() {
            return Some(actual.len().min(expected.len()));
        }
        actual.iter().zip(expected).position(|(a, b)| a != b)
    }

    fn vectors_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vectors")
    }

    /// The cells of the golden image matrix, `vectors/image/manifest.json`,
    /// each as a map from field name to value.
    ///
    /// Each cell is a flat object whose values are strings and integers
    /// without commas, colons or braces, so splitting on those characters
    /// reads it without a JSON dependency. Every cell must have exactly the
    /// known fields, so a change to the format fails here instead of being
    /// misread.
    fn golden_cells() -> Vec<std::collections::BTreeMap<String, String>> {
        const FIELDS: [&str; 10] = [
            "alpha",
            "file",
            "genome",
            "genome_hex",
            "ground_shadow",
            "mood",
            "size_px",
            "tree",
            "type",
            "version",
        ];
        let manifest = std::fs::read_to_string(vectors_root().join("image/manifest.json"))
            .expect("vectors/image/manifest.json is readable");
        let (_, entries) = manifest
            .split_once("\"entries\"")
            .expect("the manifest lists entries");
        entries
            .split('{')
            .skip(1)
            .map(|object| {
                let (fields, _) = object.split_once('}').expect("each entry is closed");
                let cell: std::collections::BTreeMap<String, String> = fields
                    .split(',')
                    .map(|field| {
                        let (key, value) = field.split_once(':').expect("a field is key: value");
                        let unquote = |text: &str| text.trim().trim_matches('"').to_owned();
                        (unquote(key), unquote(value))
                    })
                    .collect();
                assert_eq!(cell.keys().collect::<Vec<_>>(), FIELDS, "{fields}");
                cell
            })
            .collect()
    }

    fn read_genome(path: &std::path::Path) -> Vec<u8> {
        let hex = std::fs::read_to_string(path).expect("the genome file is readable");
        let hex = hex.trim().as_bytes();
        assert_eq!(hex.len() % 2, 0, "{}", path.display());
        hex.chunks(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    #[test]
    fn every_golden_image_cell_returns_the_pixels_render_image_draws() {
        // avatar-vectors pins avatar::render_image to the committed PNGs for
        // every cell, so equal bytes here mean the C call draws the golden
        // images too.
        let cells = golden_cells();
        let mut flags_seen = std::collections::BTreeSet::new();
        let mut types_seen = std::collections::BTreeSet::new();
        for cell in &cells {
            let label = &cell["file"];
            let genome = read_genome(&vectors_root().join(&cell["genome_hex"]));
            let version: u32 = cell["version"].parse().unwrap();
            let size_px: u32 = cell["size_px"].parse().unwrap();
            let (drop_ground_shadow, ground_shadow) = match cell["ground_shadow"].as_str() {
                "keep" => (0, GroundShadow::Keep),
                "drop" => (1, GroundShadow::Drop),
                other => panic!("{label}: unknown ground_shadow {other}"),
            };
            let (straight_alpha, alpha) = match cell["alpha"].as_str() {
                "premultiplied" => (0, Alpha::Premultiplied),
                "straight" => (1, Alpha::Straight),
                other => panic!("{label}: unknown alpha {other}"),
            };
            let (_, mood) = MOODS
                .into_iter()
                .find(|(name, _)| *name == cell["mood"])
                .unwrap_or_else(|| panic!("{label}: unknown mood"));
            let options = ImageOptions {
                ground_shadow,
                alpha,
            };
            let expected = render_image(&genome, &cell["type"], version, mood, size_px, &options)
                .unwrap_or_else(|error| panic!("{label}: {error}"));

            let (status, pixels, len) = render_image_raw(
                &genome,
                &cell["type"],
                version,
                &cell["mood"],
                size_px,
                drop_ground_shadow,
                straight_alpha,
            );
            assert_eq!(status, OK, "{label}");
            assert_eq!(len, size_px as usize * size_px as usize * 4, "{label}");
            let actual = take_pixels(pixels, len);
            if let Some(offset) = first_difference(&actual, &expected.pixels) {
                panic!("{label}: the C image differs from render_image at byte {offset}");
            }
            flags_seen.insert((drop_ground_shadow, straight_alpha));
            types_seen.insert(cell["type"].clone());
        }
        // A swapped or ignored flag passes unless the matrix sets each flag
        // both ways.
        assert_eq!(flags_seen.len(), 4);
        assert!(types_seen.contains(avatar::CLASSIC));
        assert!(types_seen.contains(avatar::WILD));
        println!("the C image call matched {} golden cells", cells.len());
    }

    #[test]
    fn invalid_image_arguments_return_invalid_argument_and_clear_both_outputs() {
        let classic = [0x5a; 1_024];
        for (label, size_px, drop_ground_shadow, straight_alpha) in [
            ("size 0", 0, 0, 0),
            ("size 2,049", 2_049, 0, 0),
            ("size u32::MAX", u32::MAX, 0, 0),
            ("ground shadow flag 2", 256, 2, 0),
            ("alpha flag 2", 256, 0, 2),
        ] {
            let (status, pixels, len) = render_image_raw(
                &classic,
                avatar::CLASSIC,
                2,
                "neutral",
                size_px,
                drop_ground_shadow,
                straight_alpha,
            );
            assert_eq!(status, INVALID_ARGUMENT, "{label}");
            assert!(pixels.is_null(), "{label}");
            assert_eq!(len, 0, "{label}");
        }

        // Either output missing is refused, and the one that was given is
        // still cleared.
        let avatar_type = CString::new(avatar::CLASSIC).unwrap();
        let mood = CString::new("neutral").unwrap();
        let mut pixels = ptr::dangling_mut();
        let mut len = UNSET_LEN;
        unsafe {
            let status = avatar_render_image(
                classic.as_ptr(),
                classic.len(),
                avatar_type.as_ptr(),
                2,
                mood.as_ptr(),
                256,
                0,
                0,
                ptr::null_mut(),
                &mut len,
            );
            assert_eq!(status, INVALID_ARGUMENT);
            assert_eq!(len, 0);

            let status = avatar_render_image(
                classic.as_ptr(),
                classic.len(),
                avatar_type.as_ptr(),
                2,
                mood.as_ptr(),
                256,
                0,
                0,
                &mut pixels,
                ptr::null_mut(),
            );
            assert_eq!(status, INVALID_ARGUMENT);
            assert!(pixels.is_null());

            // Null inputs are refused the same way.
            pixels = ptr::dangling_mut();
            len = UNSET_LEN;
            let status = avatar_render_image(
                ptr::null(),
                1_024,
                ptr::null(),
                2,
                ptr::null(),
                256,
                0,
                0,
                &mut pixels,
                &mut len,
            );
            assert_eq!(status, INVALID_ARGUMENT);
            assert!(pixels.is_null());
            assert_eq!(len, 0);

            avatar_image_free(ptr::null_mut(), 0);
        }
    }

    #[test]
    fn image_sizes_1_and_2_048_are_accepted() {
        let classic = [0x5a; 1_024];
        for size_px in [1, 2_048] {
            let (status, pixels, len) =
                render_image_raw(&classic, avatar::CLASSIC, 2, "neutral", size_px, 0, 0);
            assert_eq!(status, OK, "size {size_px}");
            let expected = size_px as usize * size_px as usize * 4;
            assert_eq!(take_pixels(pixels, len).len(), expected, "size {size_px}");
        }
    }

    #[test]
    fn image_refusals_check_the_row_before_the_size_and_clear_both_outputs() {
        // A stored avatar whose type, version or genome length this build
        // refuses is reported as such, never as a bad size, so each of these
        // asks for a size the call would also refuse.
        for (bytes, avatar_type, version, mood, size_px, expected) in [
            (
                &[0; 16_384][..],
                "creature.newer",
                1,
                "neutral",
                0,
                UNKNOWN_TYPE,
            ),
            (
                &[0; 1_024][..],
                avatar::CLASSIC,
                999,
                "neutral",
                2_049,
                UNKNOWN_VERSION,
            ),
            (
                &[0; 1_023][..],
                avatar::CLASSIC,
                2,
                "neutral",
                0,
                GENOME_LENGTH,
            ),
            (
                &[0; 1_024][..],
                avatar::CLASSIC,
                2,
                "unknown",
                256,
                UNKNOWN_MOOD,
            ),
        ] {
            let (status, pixels, len) =
                render_image_raw(bytes, avatar_type, version, mood, size_px, 0, 0);
            assert_eq!(status, expected, "{avatar_type} {version} {mood} {size_px}");
            assert!(pixels.is_null());
            assert_eq!(len, 0);
        }
    }

    #[test]
    fn a_second_image_call_leaves_the_first_image_intact() {
        let first_genome = [7; 16_384];
        let second_genome = [0x5a; 16_384];
        let expected = render_image(
            &first_genome,
            avatar::WILD,
            2,
            Mood::Happy,
            64,
            &ImageOptions::default(),
        )
        .unwrap()
        .pixels;

        let (status, first, first_len) =
            render_image_raw(&first_genome, avatar::WILD, 2, "happy", 64, 0, 0);
        assert_eq!(status, OK);
        // The second call draws a different creature at a different size
        // with different options while the first image is still held.
        let (status, second, second_len) =
            render_image_raw(&second_genome, avatar::WILD, 2, "sad", 96, 1, 1);
        assert_eq!(status, OK);
        assert_ne!(first, second);

        let first_pixels = take_pixels(first, first_len);
        assert!(first_difference(&first_pixels, &expected).is_none());
        assert_eq!(take_pixels(second, second_len).len(), 96 * 96 * 4);
    }
}
