//! The canonical bytes a robot's mesh digest hashes.

use super::{Robot, Skin};
use crate::mesh::MeshError;

/// A robot's rest geometry as canonical bytes, for hashing. For each part
/// in order: its mesh's canonical bytes ([`crate::mesh::Mesh::canonical_bytes`]:
/// positions, normals, projection coordinates, triangle indices and one
/// textured-front flag per triangle), then its skin: for a rigid part the
/// u32 0 and its bone, for a skinned part the u32 1 and each vertex's four
/// joint indices (u32) and four weights (f64). Then, for each texture, its
/// binding's part (u32), origin (two f64) and canvas edge C (u32). Texture
/// pixels are left out: the rasterizer may differ slightly between
/// platforms. Integers are little-endian u32 and floats little-endian f64
/// with −0.0 written as +0.0; a non-finite value is an error.
pub fn mesh_bytes(robot: &Robot) -> Result<Vec<u8>, MeshError> {
    let mut out = Vec::new();
    let u32le = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
    let f64le = |out: &mut Vec<u8>, v: f64| -> Result<(), MeshError> {
        if !v.is_finite() {
            return Err(MeshError::NonFinite);
        }
        let v = if v == 0.0 { 0.0 } else { v };
        out.extend_from_slice(&v.to_le_bytes());
        Ok(())
    };
    for part in &robot.parts {
        out.extend(part.mesh.canonical_bytes()?);
        match &part.skin {
            Skin::Rigid { bone } => {
                u32le(&mut out, 0);
                u32le(&mut out, *bone as u32);
            }
            Skin::Skinned { joints, weights } => {
                u32le(&mut out, 1);
                for (j, w) in joints.iter().zip(weights) {
                    for v in j {
                        u32le(&mut out, *v);
                    }
                    for v in w {
                        f64le(&mut out, *v)?;
                    }
                }
            }
        }
    }
    for texture in &robot.textures {
        let b = &texture.binding;
        u32le(&mut out, b.part as u32);
        f64le(&mut out, b.origin[0])?;
        f64le(&mut out, b.origin[1])?;
        u32le(&mut out, b.extent);
    }
    Ok(out)
}
