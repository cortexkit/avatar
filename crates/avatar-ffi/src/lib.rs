//! Allocation-owning C boundary for the canonical Rust render-tree pipeline.

use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

use avatar_svg::{render_tree_for_bytes, serialize_tree_json, DispatchError, Mood};

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
        let mood = match CStr::from_ptr(mood_cstr).to_bytes() {
            b"neutral" => Mood::Neutral,
            b"working" => Mood::Working,
            b"happy" => Mood::Happy,
            b"waiting" => Mood::Waiting,
            b"blocked" => Mood::Blocked,
            b"sad" => Mood::Sad,
            b"sleepy" => Mood::Sleepy,
            _ => return UNKNOWN_MOOD,
        };
        let bytes = std::slice::from_raw_parts(genome_ptr, genome_len);
        let tree = match render_tree_for_bytes(bytes, avatar_type, version, mood) {
            Ok(tree) => tree,
            Err(DispatchError::UnsupportedType { .. }) => return UNKNOWN_TYPE,
            Err(DispatchError::UnsupportedVersion { .. }) => return UNKNOWN_VERSION,
            Err(DispatchError::GenomeLength { .. }) => return GENOME_LENGTH,
            Err(DispatchError::Render(_)) => return RENDER_ERROR,
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
            let (status, output) = render(&bytes, avatar_svg::CLASSIC, 2, name);
            assert_eq!(status, OK);
            assert!(!output.is_null());
            let expected = serialize_tree_json(
                &render_tree_for_bytes(&bytes, avatar_svg::CLASSIC, 2, mood).unwrap(),
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
                avatar_svg::CLASSIC,
                999,
                "neutral",
                UNKNOWN_VERSION,
            ),
            (
                &[0; 1_023][..],
                avatar_svg::CLASSIC,
                2,
                "neutral",
                GENOME_LENGTH,
            ),
            (
                &[0; 1_024][..],
                avatar_svg::CLASSIC,
                2,
                "unknown",
                UNKNOWN_MOOD,
            ),
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
        let (status, output) = render(&newer_type_genome, avatar_svg::CLASSIC, 2, "neutral");
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
        std::thread::scope(|scope| {
            for seed in 0..8 {
                scope.spawn(move || {
                    let (status, output) = render(&[seed; 1_024], avatar_svg::CLASSIC, 2, "happy");
                    assert_eq!(status, OK);
                    unsafe { avatar_string_free(output) };
                });
            }
        });
    }
}
