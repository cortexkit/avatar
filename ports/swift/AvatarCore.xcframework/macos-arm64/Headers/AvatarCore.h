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

#ifdef __cplusplus
}
#endif

#endif
