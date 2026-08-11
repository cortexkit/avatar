use crate::Genome;

const IDENTITY_DOMAIN: &[u8] = b"cortexkit-avatar/identity-core/v1";

/// The durable source used to derive a seat's initial genome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeatIdentity {
    Registered {
        seat_name: String,
        project_id: String,
    },
    PathFallback {
        seat_name: String,
        canonical_path: String,
    },
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
        SeatIdentity::Registered {
            seat_name,
            project_id,
        } => {
            hasher.update(seat_name.as_bytes());
            hasher.update(&[0]);
            hasher.update(project_id.as_bytes());
        }
        SeatIdentity::PathFallback {
            seat_name,
            canonical_path,
        } => {
            hasher.update(b"path-fallback");
            hasher.update(&[0]);
            hasher.update(seat_name.as_bytes());
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
            seat_name: "mason".to_owned(),
            project_id: "project-42".to_owned(),
        });
        let fallback = derive_genome(&SeatIdentity::PathFallback {
            seat_name: "mason".to_owned(),
            canonical_path: "project-42".to_owned(),
        });
        let empty = derive_genome(&SeatIdentity::Registered {
            seat_name: String::new(),
            project_id: String::new(),
        });

        assert_eq!(
            &registered.to_hex()[..64],
            "6d4604f3c6319289abbcb60a9ad68bc1d9e49746176c58a65528612de3ceb26b"
        );
        assert_eq!(
            &fallback.to_hex()[..64],
            "79dcf9c284a2022285915902e2c85f1b61bbdc992fecdb3f30d88a1d216f54ea"
        );
        assert_eq!(
            &empty.to_hex()[..64],
            "266c4534796028fb23cbff26a90574f18adbfba9f17361a40d167babdc87cb63"
        );
    }

    #[test]
    fn fallback_and_registered_are_domain_separated() {
        let registered = derive_genome(&SeatIdentity::Registered {
            seat_name: "same".to_owned(),
            project_id: "value".to_owned(),
        });
        let fallback = derive_genome(&SeatIdentity::PathFallback {
            seat_name: "same".to_owned(),
            canonical_path: "value".to_owned(),
        });

        assert_ne!(registered, fallback);
    }

    #[test]
    fn changing_the_seat_name_changes_every_stream() {
        let first = derive_genome(&SeatIdentity::Registered {
            seat_name: "first".to_owned(),
            project_id: "project".to_owned(),
        });
        let second = derive_genome(&SeatIdentity::Registered {
            seat_name: "second".to_owned(),
            project_id: "project".to_owned(),
        });

        assert_ne!(first, second);
    }
}
