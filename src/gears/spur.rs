use std::f64::consts::{PI, TAU};
use serde::Deserialize;

use crate::commands::{create_extruded_gear, resolve_smart_frame};
use crate::craft::plugin::types::PluginError;
use crate::gears::iso::{
    dedup_contour, parse_iso_module, polar, IsoGearDimensions, Precision, Pt2,
};

#[derive(Deserialize)]
pub struct SpurGearParams {
    pub teeth: usize,
    pub iso_module: String,
    #[serde(default = "default_pressure_angle")]
    pub pressure_angle_deg: f64,
    #[serde(default)]
    pub profile_shift: f64,
    #[serde(default = "default_true")]
    pub extrude: bool,
    #[serde(default = "default_thickness")]
    pub thickness_mm: f64,
    #[serde(default)]
    pub bore_diameter_mm: f64,
    #[serde(default = "default_backlash")]
    pub backlash_mm: f64,
    #[serde(default = "default_precision")]
    pub precision: String,
    #[serde(default)]
    pub force_origin: bool,
    #[serde(default = "default_plane")]
    pub fallback_plane: String,
    #[serde(default)]
    pub offset_x_mm: f64,
    #[serde(default)]
    pub offset_y_mm: f64,
    #[serde(default)]
    pub offset_z_mm: f64,
}

fn default_pressure_angle() -> f64 { 20.0 }
fn default_true() -> bool { true }
fn default_thickness() -> f64 { 10.0 }
fn default_backlash() -> f64 { 0.04 }
fn default_precision() -> String { "medium".into() }
fn default_plane() -> String { "XY".into() }

pub fn build_involute_contour(
    dims: &IsoGearDimensions,
    backlash_mm: f64,
    precision: Precision,
) -> Vec<Pt2> {
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
        (PI / (2.0 * z)) + (profile_shift_arc - backlash_mm) / (2.0 * r_pitch);
    let half_tooth_base_angle = half_tooth_pitch_angle + inv_alpha_t;

    let t_max = ((r_addendum / r_base).powi(2) - 1.0).max(0.0).sqrt();
    let t_min = if r_root > r_base {
        ((r_root / r_base).powi(2) - 1.0).sqrt()
    } else {
        0.0
    };

    let samples = precision.involute_samples();
    let step = TAU / z;
    let mut pts = Vec::with_capacity(dims.teeth * (samples * 2 + 6));

    for i in 0..dims.teeth {
        let center_angle = (i as f64) * step;

        let root_start_angle = center_angle - step / 2.0;
        pts.push(polar(r_root, root_start_angle));

        if r_root < r_base {
            pts.push(polar(r_root, center_angle - half_tooth_base_angle));
            pts.push(polar(r_base, center_angle - half_tooth_base_angle));
        }

        for s in 0..=samples {
            let t = t_min + (t_max - t_min) * (s as f64 / samples as f64);
            let r = r_base * (1.0 + t * t).sqrt();
            let inv_t = t - t.atan();
            let theta = center_angle - half_tooth_base_angle + inv_t;
            pts.push(polar(r, theta));
        }

        for s in (0..=samples).rev() {
            let t = t_min + (t_max - t_min) * (s as f64 / samples as f64);
            let r = r_base * (1.0 + t * t).sqrt();
            let inv_t = t - t.atan();
            let theta = center_angle + half_tooth_base_angle - inv_t;
            pts.push(polar(r, theta));
        }

        if r_root < r_base {
            pts.push(polar(r_base, center_angle + half_tooth_base_angle));
            pts.push(polar(r_root, center_angle + half_tooth_base_angle));
        }
    }

    dedup_contour(pts, 1e-4)
}

pub fn run(p: SpurGearParams) -> Result<(String, f64), PluginError> {
    let m = parse_iso_module(&p.iso_module).map_err(|e| crate::invalid_field("iso_module", e))?;
    let dims = IsoGearDimensions::compute(p.teeth, m, p.pressure_angle_deg, 0.0, p.profile_shift);
    crate::validate_dims(&dims, p.extrude, p.thickness_mm, p.bore_diameter_mm)?;

    let offset = [p.offset_x_mm, p.offset_y_mm, p.offset_z_mm];
    let frame = resolve_smart_frame(p.force_origin, &p.fallback_plane, offset);
    let prec = Precision::from_str(&p.precision);
    let contour = build_involute_contour(&dims, p.backlash_mm, prec);
    let base_name = format!("SpurGear_Z{}_M{}", p.teeth, p.iso_module.replace('.', "_"));

    let created = create_extruded_gear(
        &base_name,
        &frame,
        &dims,
        &contour,
        p.extrude,
        p.thickness_mm,
        p.bore_diameter_mm,
        true, // рисуем все 4 окружности: da, d, db, df + центральную точку
    )?;
    Ok((created, m))
}