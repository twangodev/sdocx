//! Typed PDF sidecar for vector ink fields that need native PDF lowering.
//!
//! The SVG remains the public visual representation. In retained-PDF scenes,
//! a stable ID connects a generated group to this data so the PDF converter
//! can replace only that group's filtered children.

use crate::{BoundingBox, CoverageMesh};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct NativeInkId(usize);

impl NativeInkId {
    pub(crate) fn svg_id(self) -> String {
        format!("sdocx-native-ink-{}", self.0)
    }

    pub(crate) fn marker_id(self, page_index: usize) -> Option<u64> {
        let page = u32::try_from(page_index).ok()?;
        let local = u32::try_from(self.0).ok()?;
        Some((u64::from(page) << 32) | u64::from(local))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct NativeInk {
    pub(crate) mesh: CoverageMesh,
    pub(crate) color: [u8; 3],
    pub(crate) bounds: BoundingBox,
    /// Kept separate from alpha in the source color, which the PDF lowering
    /// applies once at the intercepted SVG group boundary.
    pub(crate) opacity: f64,
}

#[derive(Debug, Default)]
pub(crate) struct NativeInkRegistry {
    entries: Vec<NativeInk>,
}

impl NativeInkRegistry {
    pub(crate) fn register(
        &mut self,
        mesh: &CoverageMesh,
        color: [u8; 3],
        opacity: f64,
    ) -> Result<NativeInkId, &'static str> {
        let bounds = mesh.bounds.ok_or("native ink mesh has no bounds")?;
        let [x_min, y_min, x_max, y_max] = [bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max];
        if ![x_min, y_min, x_max, y_max, opacity]
            .into_iter()
            .all(f64::is_finite)
            || x_max <= x_min
            || y_max <= y_min
            || !(0.0..=1.0).contains(&opacity)
        {
            return Err("native ink has invalid bounds or opacity");
        }
        if mesh.triangles.is_empty()
            || mesh.triangles.iter().any(|triangle| {
                triangle.vertices.iter().any(|vertex| {
                    !vertex.x.is_finite()
                        || !vertex.y.is_finite()
                        || !vertex.coverage.is_finite()
                        || !(0.0..=1.0).contains(&vertex.coverage)
                })
            })
        {
            return Err("native ink mesh has invalid coverage triangles");
        }
        let mut vertices = mesh
            .triangles
            .iter()
            .flat_map(|triangle| triangle.vertices.iter());
        let first = vertices.next().ok_or("native ink mesh has no vertices")?;
        let mut derived = BoundingBox {
            x_min: f64::from(first.x),
            y_min: f64::from(first.y),
            x_max: f64::from(first.x),
            y_max: f64::from(first.y),
        };
        for vertex in vertices {
            derived.x_min = derived.x_min.min(f64::from(vertex.x));
            derived.y_min = derived.y_min.min(f64::from(vertex.y));
            derived.x_max = derived.x_max.max(f64::from(vertex.x));
            derived.y_max = derived.y_max.max(f64::from(vertex.y));
        }
        if bounds != derived {
            return Err("native ink bounds do not match coverage mesh vertices");
        }

        let id = NativeInkId(self.entries.len());
        self.entries.push(NativeInk {
            mesh: mesh.clone(),
            color,
            bounds,
            opacity,
        });
        Ok(id)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (NativeInkId, &NativeInk)> {
        self.entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (NativeInkId(index), entry))
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CoverageTriangle, CoverageVertex};

    fn mesh() -> CoverageMesh {
        CoverageMesh {
            triangles: vec![CoverageTriangle {
                vertices: [
                    CoverageVertex {
                        x: 2.0,
                        y: 3.0,
                        coverage: 0.0,
                    },
                    CoverageVertex {
                        x: 9.0,
                        y: 3.0,
                        coverage: 0.5,
                    },
                    CoverageVertex {
                        x: 4.0,
                        y: 8.0,
                        coverage: 1.0,
                    },
                ],
            }],
            sample_ends: vec![1],
            terminal_start: 1,
            terminal_end: 1,
            bounds: Some(BoundingBox {
                x_min: 2.0,
                y_min: 3.0,
                x_max: 9.0,
                y_max: 8.0,
            }),
        }
    }

    #[test]
    fn registers_typed_meshes_with_stable_unique_svg_ids() {
        let mut registry = NativeInkRegistry::default();
        let first = registry.register(&mesh(), [0x33, 0x66, 0x99], 0.5).unwrap();
        let second = registry.register(&mesh(), [1, 2, 3], 1.0).unwrap();
        assert_eq!(first.svg_id(), "sdocx-native-ink-0");
        assert_eq!(second.svg_id(), "sdocx-native-ink-1");
        let entries = registry.iter().collect::<Vec<_>>();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].1.color, [0x33, 0x66, 0x99]);
        assert_eq!(entries[0].1.bounds.x_min, 2.0);
        assert_eq!(entries[0].1.opacity, 0.5);
    }

    #[test]
    fn rejects_invalid_domains_without_registering_them() {
        for opacity in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
            let mut registry = NativeInkRegistry::default();
            assert!(registry.register(&mesh(), [0, 0, 0], opacity).is_err());
            assert!(registry.is_empty());
        }

        let mut empty = mesh();
        empty.bounds = None;
        let mut registry = NativeInkRegistry::default();
        assert!(registry.register(&empty, [0, 0, 0], 1.0).is_err());
        assert!(registry.is_empty());

        let mut invalid = mesh();
        invalid.triangles[0].vertices[0].coverage = 1.5;
        assert!(registry.register(&invalid, [0, 0, 0], 1.0).is_err());
        assert!(registry.is_empty());

        let mut wrong_bounds = mesh();
        wrong_bounds.bounds.as_mut().unwrap().x_min = 1.0;
        assert!(registry.register(&wrong_bounds, [0, 0, 0], 1.0).is_err());
        assert!(registry.is_empty());
    }

    #[test]
    fn marker_ids_are_unique_across_pages_with_local_id_zero() {
        let local = NativeInkId(0);
        let page_zero = local.marker_id(0).unwrap();
        let page_one = local.marker_id(1).unwrap();
        assert_ne!(page_zero, page_one);
        assert_eq!(page_zero, 0);
        assert_eq!(page_one, 1_u64 << 32);
        assert!(local.marker_id(usize::MAX).is_none());
    }
}
