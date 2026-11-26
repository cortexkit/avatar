#ifndef AVATAR_CORE_H
#define AVATAR_CORE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define AVATAR_STATUS_OK 0
#define AVATAR_STATUS_UNKNOWN_TYPE 1
#define AVATAR_STATUS_UNKNOWN_VERSION 2
#define AVATAR_STATUS_GENOME_LENGTH 3
#define AVATAR_STATUS_UNKNOWN_MOOD 4
#define AVATAR_STATUS_INVALID_ARGUMENT 5
#define AVATAR_STATUS_RENDER_ERROR 6
#define AVATAR_STATUS_PANIC 7

/* Pure, reentrant rendering; no caller buffers are retained. Strings must be
 * NUL-terminated UTF-8. genome_ptr must address genome_len readable bytes.
 * out_json must address a writable pointer and must not alias any input.
 * On success it receives canonical tree JSON, owned by the caller until freed
 * with avatar_string_free. On failure it is set to NULL (if non-NULL itself).
 * NULL inputs are rejected; invalid non-NULL pointers remain caller errors. */
int32_t avatar_render_tree_json(const uint8_t *genome_ptr, size_t genome_len,
                               const char *type_cstr, uint32_t version,
                               const char *mood_cstr, char **out_json);

/* Accepts NULL, or a live string returned by avatar_render_tree_json exactly
 * once. Rust's allocator must free its own allocations, not Swift's or C's. */
void avatar_string_free(char *ptr);

/* Draws a stored avatar as a finished square image, size_px pixels on each
 * edge: sRGB RGBA8, rows top first, tightly packed. Pure and reentrant like
 * the tree call, with the same input rules. drop_ground_shadow (leave out the
 * creature's ground shadow) and straight_alpha (return straight rather than
 * premultiplied alpha) must each be 0 or 1; a NULL argument or another flag
 * value returns AVATAR_STATUS_INVALID_ARGUMENT before anything else is
 * checked. Then come the tree call's checks, in the same order, and only
 * then size_px, which must be 1 to 2048 or the call returns
 * AVATAR_STATUS_INVALID_ARGUMENT. So a row this build cannot depict is
 * reported as such whatever size was asked for. A rasterizer failure returns
 * AVATAR_STATUS_RENDER_ERROR.
 * out_pixels and out_len must be writable and must not alias each other or
 * any input. Each non-NULL output is first set to NULL or 0, and a NULL
 * output is refused, so after any failure the outputs given hold NULL and 0.
 * On success *out_pixels receives a new allocation of exactly *out_len =
 * size_px * size_px * 4 bytes, owned by the caller until freed with
 * avatar_image_free. No later call invalidates it. */
int32_t avatar_render_image(const uint8_t *genome_ptr, size_t genome_len,
                            const char *type_cstr, uint32_t version,
                            const char *mood_cstr, uint32_t size_px,
                            uint32_t drop_ground_shadow,
                            uint32_t straight_alpha, uint8_t **out_pixels,
                            size_t *out_len);

/* Accepts NULL (any len), or a live pointer returned by avatar_render_image
 * with exactly the length returned with it, exactly once. */
void avatar_image_free(uint8_t *pixels, size_t len);

#ifdef __cplusplus
}
#endif

#endif
