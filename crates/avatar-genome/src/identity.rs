use crate::Genome;

const IDENTITY_DOMAIN: &[u8] = b"cortexkit-avatar/identity-core/v2";

/// The durable source used to derive a seat's initial genome.
///
/// Live-fleet measurement found every candidate name source user-editable,
/// non-unique, or session-derived. Hashing a name would make a rename re-roll
/// a creature and let a collision merge creatures, so names remain presentation
/// only. Identity is the registry's one opaque, stable value; adding a name to
/// this enum or its derivation would reintroduce those failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeatIdentity {
    Registered { project_id: String },
    PathFallback { canonical_path: String },
}

/// Derives a complete initial genome from a seat identity.
///
/// Domain separation keeps this XOF stream distinct from every other fleet
/// hash. Path fallback adds its own prefix so a mutable path can never produce
/// the same initial DNA as a registered project identifier with equal bytes.
pub fn derive_genome(identity: &SeatIdentity) -> Genome {
    let mut hasher = blake3::Hasher::new();
    hasher.update(IDENTITY_DOMAIN);
    hasher.update(&[0]);

    match identity {
        SeatIdentity::Registered { project_id } => {
            hasher.update(project_id.as_bytes());
        }
        SeatIdentity::PathFallback { canonical_path } => {
            hasher.update(b"path-fallback");
            hasher.update(&[0]);
            hasher.update(canonical_path.as_bytes());
        }
    }

    let mut bytes = [0; 1_024];
    hasher.finalize_xof().fill(&mut bytes);
    Genome::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_prefixes_are_compatibility_pins() {
        let registered = derive_genome(&SeatIdentity::Registered {
            project_id: "project-42".to_owned(),
        });
        let fallback = derive_genome(&SeatIdentity::PathFallback {
            canonical_path: "project-42".to_owned(),
        });
        let empty = derive_genome(&SeatIdentity::Registered {
            project_id: String::new(),
        });

        assert_eq!(
            &registered.to_hex()[..64],
            "497c376676efcfbabd57a41123143dbb72df26ad5f2b09c6d7331410ea13c289"
        );
        assert_eq!(
            &fallback.to_hex()[..64],
            "70e579fc8f565c4563aeccb739a43eb9867e2264e6cb2a658adc8621edb63cd3"
        );
        assert_eq!(
            &empty.to_hex()[..64],
            "ac844c9c9582832925ef89a4b581d9f31d23d5b69702fd408f5e1a2167d5bfcb"
        );
    }

    #[test]
    fn fallback_and_registered_are_domain_separated() {
        let registered = derive_genome(&SeatIdentity::Registered {
            project_id: "value".to_owned(),
        });
        let fallback = derive_genome(&SeatIdentity::PathFallback {
            canonical_path: "value".to_owned(),
        });

        assert_ne!(registered, fallback);
    }

    #[test]
    fn different_ids_derive_different_genomes() {
        let first = derive_genome(&SeatIdentity::Registered {
            project_id: "first".to_owned(),
        });
        let second = derive_genome(&SeatIdentity::Registered {
            project_id: "second".to_owned(),
        });

        assert_ne!(first, second);
    }
}
