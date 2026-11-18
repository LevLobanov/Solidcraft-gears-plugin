use std::f64::consts::TAU;
use serde::Deserialize;

use crate::commands::{create_extruded_gear, resolve_smart_frame};
use crate::craft::plugin::types::PluginError;
use crate::gears::iso::{dedup_contour, parse_iso_module, polar, IsoGearDimensions, Pt2};

#[derive(Deserialize)]
pub struct StandardGearParams {
    pub teeth: usize,
    pub iso_module: String,
    #[serde(default = "default_true")]
    pub extrude: bool,
    #[serde(default = "default_thickness")]
    pub thickness_mm: f64,
    #[serde(default)]
    pub bore_diameter_mm: f64,
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

fn default_true() -> bool { true }
fn default_thickness() -> f64 { 10.0 }
fn default_plane() -> String { "XY".into() }

pub fn build_contour(dims: &IsoGearDimensions) -> Vec<Pt2> {
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

    dedup_contour(pts, 1e-4)
}

pub fn run(p: StandardGearParams) -> Result<(String, f64), PluginError> {
    let m = parse_iso_module(&p.iso_module).map_err(|e| crate::invalid_field("iso_module", e))?;
    let dims = IsoGearDimensions::compute(p.teeth, m, 20.0, 0.0, 0.0);
    crate::validate_dims(&dims, p.extrude, p.thickness_mm, p.bore_diameter_mm)?;

    let offset = [p.offset_x_mm, p.offset_y_mm, p.offset_z_mm];
    let frame = resolve_smart_frame(p.force_origin, &p.fallback_plane, offset);
    let contour = build_contour(&dims);
    let base_name = format!("StandardGear_Z{}_M{}", p.teeth, p.iso_module.replace('.', "_"));

    let created = create_extruded_gear(
        &base_name,
        &frame,
        &dims,
        &contour,
        p.extrude,
        p.thickness_mm,
        p.bore_diameter_mm,
        false, // для упрощённой трапециевидной шестерни основная окружность эвольвенты не нужна
    )?;

    Ok((created, m))
}