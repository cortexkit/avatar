# Vendored BLAKE3 reference C implementation

Upstream: https://github.com/BLAKE3-team/BLAKE3 (directory `c/`)
Vendored commit: `77b257eee7da5cd608eaf6be8343d3a4c9776af2` (release 1.8.6)
License: CC0 1.0 Universal (public domain), as stated in the upstream README.

| File (in this target) | Upstream path | sha256 |
| --- | --- | --- |
| `blake3.c` | `c/blake3.c` | `b118ddf7cf9e6e5ef3fded72dcb1acf9dfdc4ea923cbe4605900ad6ee9afe1af` |
| `blake3_dispatch.c` | `c/blake3_dispatch.c` | `134f21550138c0af6312925c988aeee35df287e4119e8ad1d206fccdb2238fe3` |
| `blake3_portable.c` | `c/blake3_portable.c` | `2bc25b0dad67b4329d0b49cfa075ab2b0d04e424addbddc4e9c389c52a192524` |
| `blake3_impl.h` | `c/blake3_impl.h` | `d388dca3574602c8849805ea3c8c0a12d082ac0e428756f305e111942f099af4` |
| `include/blake3.h` | `c/blake3.h` | `905b20212f4ecd6bfd71b6d1642a46546f33e7c9807a5a18635b4bae555b965d` |

`include/blake3.h` is the target's public header (SwiftPM `publicHeadersPath`);
the other four files are compiled but not exported.

## Portable-only build

The package manifest compiles this target with `BLAKE3_NO_SSE2`,
`BLAKE3_NO_SSE41`, `BLAKE3_NO_AVX2`, `BLAKE3_NO_AVX512`, and
`BLAKE3_USE_NEON=0` so that only `blake3_portable.c` is reachable on every
architecture. Determinism and buildability on any host beat throughput here
(avatars hash 1 KiB payloads); the SIMD sources are deliberately not vendored.
Note the asymmetry in the upstream guards: the x86 paths are disabled by the
`BLAKE3_NO_*` macros, but the NEON path keys off `BLAKE3_USE_NEON`, which
upstream autodetects as 1 on AArch64 unless manually overridden — hence the
explicit `BLAKE3_USE_NEON=0` rather than a `BLAKE3_NO_NEON` define alone.

## API note

Upstream 1.8.6 exposes the XOF through `blake3_hasher_finalize_seek`
(`blake3_hasher_finalize` is `finalize_seek` with seek 0); earlier docs name
the same operation `finalize_xof`. Genome derivation uses it to fill exactly
1 KiB from the root output stream, matching the Rust crate's
`Hasher::finalize_xof().fill(...)`.
