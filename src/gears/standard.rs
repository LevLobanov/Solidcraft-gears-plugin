use std::f64::consts::TAU;

use crate::gears::{
    dedup_contour, polar, GearProfile2D, GearSpec, IsoGearDimensions, ValidationError,
};

pub fn generate(spec: &GearSpec) -> Result<GearProfile2D, ValidationError> {
    let dims = IsoGearDimensions::compute(spec);
    dims.validate(spec)?;

    let z = dims.teeth as f64;
    let step = TAU / z;
    let mut pts = Vec::with_capacity(dims.teeth * 5);

    for i in 0..dims.teeth {
        let base = (i as f64) * step;
        pts.push(polar(dims.root_radius, base));
        pts.push(polar(dims.root_radius, base + step * 0.18));
        pts.push(polar(dims.addendum_radius, base + step * 0.32));
        pts.push(polar(dims.addendum_radius, base + step * 0.68));
        pts.push(polar(dims.root_radius, base + step * 0.82));
    }

    Ok(GearProfile2D {
        dims,
        outer_contour: dedup_contour(pts, 2e-3),
        bore_diameter_mm: (spec.bore_diameter_mm > 0.0).then_some(spec.bore_diameter_mm),
        reference_diameters_mm: vec![
            dims.addendum_radius * 2.0,
            dims.pitch_radius * 2.0,
            dims.root_radius * 2.0,
        ],
        center: [0.0, 0.0],
    })
}