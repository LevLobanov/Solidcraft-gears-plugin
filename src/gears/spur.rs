use std::f64::consts::{PI, TAU};

use crate::gears::{
    dedup_contour, polar, GearProfile2D, GearSpec, IsoGearDimensions, ValidationError,
};

pub fn generate(spec: &GearSpec) -> Result<GearProfile2D, ValidationError> {
    let dims = IsoGearDimensions::compute(spec);
    dims.validate(spec)?;

    let z = dims.teeth as f64;
    let alpha_t = dims.transverse_pressure_angle_rad;
    let r_pitch = dims.pitch_radius;
    let r_base = dims.base_radius;
    let r_addendum = dims.addendum_radius;
    let r_root = dims.root_radius;

    let inv_alpha_t = alpha_t.tan() - alpha_t;
    let profile_shift_arc =
        2.0 * dims.profile_shift * dims.normal_module * dims.normal_pressure_angle_rad.tan();
    let half_tooth_pitch_angle =
        (PI / (2.0 * z)) + (profile_shift_arc - spec.backlash_mm) / (2.0 * r_pitch);
    let half_tooth_base_angle = half_tooth_pitch_angle + inv_alpha_t;

    let t_max = ((r_addendum / r_base).powi(2) - 1.0).max(0.0).sqrt();
    let t_min = if r_root > r_base {
        ((r_root / r_base).powi(2) - 1.0).sqrt()
    } else {
        0.0
    };

    let samples = spec.precision.involute_samples();
    let step = TAU / z;
    let mut pts = Vec::with_capacity(dims.teeth * (samples * 2 + 6));

    for i in 0..dims.teeth {
        let center_angle = (i as f64) * step;
        pts.push(polar(r_root, center_angle - step / 2.0));

        if r_root < r_base {
            pts.push(polar(r_root, center_angle - half_tooth_base_angle));
            pts.push(polar(r_base, center_angle - half_tooth_base_angle));
        }

        for s in 0..=samples {
            let t = t_min + (t_max - t_min) * (s as f64 / samples as f64);
            let r = r_base * (1.0 + t * t).sqrt();
            let inv_t = t - t.atan();
            pts.push(polar(r, center_angle - half_tooth_base_angle + inv_t));
        }

        for s in (0..=samples).rev() {
            let t = t_min + (t_max - t_min) * (s as f64 / samples as f64);
            let r = r_base * (1.0 + t * t).sqrt();
            let inv_t = t - t.atan();
            pts.push(polar(r, center_angle + half_tooth_base_angle - inv_t));
        }

        if r_root < r_base {
            pts.push(polar(r_base, center_angle + half_tooth_base_angle));
            pts.push(polar(r_root, center_angle + half_tooth_base_angle));
        }
    }

    Ok(GearProfile2D {
        dims,
        outer_contour: dedup_contour(pts, 2e-3),
        bore_diameter_mm: (spec.bore_diameter_mm > 0.0).then_some(spec.bore_diameter_mm),
        reference_diameters_mm: vec![
            dims.addendum_radius * 2.0,
            dims.pitch_radius * 2.0,
            dims.base_radius * 2.0,
            dims.root_radius * 2.0,
        ],
        center: [0.0, 0.0],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gears::{GearKind, Precision};

    #[test]
    fn spur_contour_has_no_zero_length_segments() {
        for module_mm in [0.5, 2.0] {
            for prec in [Precision::Low, Precision::Medium, Precision::High, Precision::Ultra] {
                let spec = GearSpec {
                    kind: GearKind::Spur,
                    teeth: 24,
                    module_mm,
                    module_label: module_mm.to_string(),
                    pressure_angle_deg: 20.0,
                    profile_shift: 0.0,
                    backlash_mm: 0.04,
                    precision: prec,
                    extrude: true,
                    thickness_mm: 10.0,
                    bore_diameter_mm: 0.0,
                };
                let profile = generate(&spec).unwrap();
                for w in profile.outer_contour.windows(2) {
                    let d = ((w[0][0] - w[1][0]).powi(2) + (w[0][1] - w[1][1]).powi(2)).sqrt();
                    assert!(d >= 2e-3, "Segment too short: {d}");
                }
                let first = profile.outer_contour[0];
                let last = *profile.outer_contour.last().unwrap();
                let close_d = ((first[0] - last[0]).powi(2) + (first[1] - last[1]).powi(2)).sqrt();
                assert!(close_d >= 2e-3, "Closing segment too short: {close_d}");
            }
        }
    }
}