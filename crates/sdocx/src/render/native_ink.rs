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
pub(crate) enum NativeInkPayload {
    LegacyMarker {
        form: Box<super::legacy_marker_vector::pdf::Prepared>,
    },
    Mesh {
        mesh: CoverageMesh,
        color: [u8; 3],
    },
    Radial {
        records: Vec<[f32; 9]>,
        saved_alpha: f32,
        enhanced_antialias: bool,
    },
    RadialCircles {
        records: Vec<[f32; 9]>,
        saved_alpha: f32,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct NativeInk {
    pub(crate) payload: NativeInkPayload,
    pub(crate) bounds: BoundingBox,
    /// Applied once at the intercepted SVG group boundary. Enhanced radial
    /// fields use saved alpha here and in their color recurrence; direct fields
    /// use opacity 1 because their mask already contains per-stamp saved alpha.
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
            payload: NativeInkPayload::Mesh {
                mesh: mesh.clone(),
                color,
            },
            bounds,
            opacity,
        });
        Ok(id)
    }

    pub(crate) fn register_legacy_marker(
        &mut self,
        form: super::legacy_marker_vector::pdf::Prepared,
        bounds: BoundingBox,
    ) -> NativeInkId {
        let id = NativeInkId(self.entries.len());
        self.entries.push(NativeInk {
            payload: NativeInkPayload::LegacyMarker {
                form: Box::new(form),
            },
            bounds,
            opacity: 1.,
        });
        id
    }

    pub(crate) fn register_radial(
        &mut self,
        records: &[[f32; 9]],
        bounds: BoundingBox,
        saved_alpha: f32,
        enhanced_antialias: bool,
    ) -> Result<NativeInkId, String> {
        // Preflight the exact compiler before publishing an SVG ID or cloning
        // records. An unsupported field cannot leave a partial registration.
        crate::pdf::validate_radial_ink(records, bounds, saved_alpha, enhanced_antialias)?;
        let id = NativeInkId(self.entries.len());
        self.entries.push(NativeInk {
            payload: NativeInkPayload::Radial {
                records: records.to_vec(),
                saved_alpha,
                enhanced_antialias,
            },
            bounds,
            opacity: if enhanced_antialias {
                f64::from(saved_alpha)
            } else {
                1.0
            },
        });
        Ok(id)
    }

    pub(crate) fn register_radial_circles(
        &mut self,
        records: &[[f32; 9]],
        bounds: BoundingBox,
        saved_alpha: f32,
    ) -> Result<NativeInkId, String> {
        // Only the full-coverage equal-circle branch is admitted; validate
        // before adding an ID or copying any payload.
        crate::pdf::validate_radial_circles(records, bounds, saved_alpha)?;
        let id = NativeInkId(self.entries.len());
        self.entries.push(NativeInk {
            payload: NativeInkPayload::RadialCircles {
                records: records.to_vec(),
                saved_alpha,
            },
            bounds,
            opacity: 1.0,
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
        assert!(matches!(
            &entries[0].1.payload,
            NativeInkPayload::Mesh {
                color: [0x33, 0x66, 0x99],
                ..
            }
        ));
        assert_eq!(entries[0].1.bounds.x_min, 2.0);
        assert_eq!(entries[0].1.opacity, 0.5);
    }

    #[test]
    fn radial_registration_preserves_records_and_owns_outer_alpha_once() {
        let records = [[2., 3., 1., 2., 0.25, 0.8, 1., 0., 0.]];
        let bounds = BoundingBox {
            x_min: 1.,
            y_min: 1.,
            x_max: 3.,
            y_max: 5.,
        };
        let alpha = 128.0_f32 / 255.;
        let mut registry = NativeInkRegistry::default();
        registry
            .register_radial(&records, bounds, alpha, true)
            .unwrap();
        registry
            .register_radial(&records, bounds, alpha, false)
            .unwrap();
        let entries: Vec<_> = registry.iter().collect();
        assert_eq!(entries[0].1.opacity, f64::from(alpha));
        assert_eq!(entries[1].1.opacity, 1.);
        for ((_, entry), enhanced) in entries.iter().zip([true, false]) {
            let NativeInkPayload::Radial {
                records: retained,
                saved_alpha,
                enhanced_antialias,
            } = &entry.payload
            else {
                panic!("expected radial field");
            };
            assert_eq!(retained, &records);
            assert_eq!(*saved_alpha, alpha);
            assert_eq!(*enhanced_antialias, enhanced);
        }
    }

    #[test]
    fn radial_preflight_failure_leaves_registry_and_ids_untouched() {
        let mut registry = NativeInkRegistry::default();
        let invalid = [[2., 3., 1., 2., 0.5, 0.8, 1., 0., 0.]];
        let bounds = BoundingBox {
            x_min: 1.,
            y_min: 1.,
            x_max: 3.,
            y_max: 5.,
        };
        assert!(
            registry
                .register_radial(&invalid, bounds, 0.5, true)
                .is_err()
        );
        assert!(registry.is_empty());
        let first = registry.register(&mesh(), [1, 2, 3], 1.).unwrap();
        assert_eq!(first.svg_id(), "sdocx-native-ink-0");
    }

    #[test]
    fn circle_preflight_failure_does_not_register_a_partial_stroke() {
        let mut registry = NativeInkRegistry::default();
        let records = [[2.0, 3.0, 1.0, 1.0, 0.0, 0.5, 1.0, 0.0, 0.0]];
        let bounds = BoundingBox {
            x_min: 1.0,
            y_min: 1.0,
            x_max: 3.0,
            y_max: 5.0,
        };
        assert!(
            registry
                .register_radial_circles(&records, bounds, 0.5)
                .is_err()
        );
        assert!(registry.is_empty());
        let next = registry.register(&mesh(), [1, 2, 3], 1.0).unwrap();
        assert_eq!(next.svg_id(), "sdocx-native-ink-0");
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
