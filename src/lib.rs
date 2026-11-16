mod commands;
mod geometry;

wit_bindgen::generate!({
    path: "wit",
    world: "command-plugin",
});

use commands::{create_gear, resolve_smart_frame};
use craft::plugin::log::{self, Level};
use craft::plugin::types::{FieldFailure, PluginError};
use exports::craft::plugin::entry::Guest;
use geometry::{build_spur_gear_contour, build_standard_gear_contour, Precision};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct StandardGearParams {
    teeth: usize,
    module_mm: f64,
    #[serde(default = "default_true")]
    extrude: bool,
    #[serde(default = "default_thickness")]
    thickness_mm: f64,
    #[serde(default)]
    bore_diameter_mm: f64,
    #[serde(default)]
    force_origin: bool,
    #[serde(default = "default_plane")]
    fallback_plane: String,
    #[serde(default)]
    offset_x_mm: f64,
    #[serde(default)]
    offset_y_mm: f64,
    #[serde(default)]
    offset_z_mm: f64,
}

#[derive(Deserialize)]
struct SpurGearParams {
    teeth: usize,
    module_mm: f64,
    #[serde(default = "default_pressure_angle")]
    pressure_angle_deg: f64,
    #[serde(default = "default_true")]
    extrude: bool,
    #[serde(default = "default_thickness")]
    thickness_mm: f64,
    #[serde(default)]
    bore_diameter_mm: f64,
    #[serde(default)]
    backlash_mm: f64,
    #[serde(default = "default_precision")]
    precision: String,
    #[serde(default)]
    force_origin: bool,
    #[serde(default = "default_plane")]
    fallback_plane: String,
    #[serde(default)]
    offset_x_mm: f64,
    #[serde(default)]
    offset_y_mm: f64,
    #[serde(default)]
    offset_z_mm: f64,
}

fn default_true() -> bool {
    true
}

fn default_thickness() -> f64 {
    10.0
}

fn default_pressure_angle() -> f64 {
    20.0
}

fn default_plane() -> String {
    "XY".to_string()
}

fn default_precision() -> String {
    "medium".to_string()
}

fn invalid_field(field: &str, reason: impl Into<String>) -> PluginError {
    PluginError::InvalidInput(FieldFailure {
        field: field.into(),
        reason: reason.into(),
    })
}

struct GearPlugin;

impl Guest for GearPlugin {
    fn execute(entry: String, params_json: String) -> Result<String, PluginError> {
        log::log(Level::Info, &format!("Executing gear entry: {entry}"));

        match entry.as_str() {
            "standard-gear" => {
                let p: StandardGearParams = serde_json::from_str(&params_json)
                    .map_err(|e| invalid_field("params", e.to_string()))?;
                validate_common(p.teeth, p.module_mm, p.extrude, p.thickness_mm, p.bore_diameter_mm)?;

                let offset = [p.offset_x_mm, p.offset_y_mm, p.offset_z_mm];
                let frame = resolve_smart_frame(p.force_origin, &p.fallback_plane, offset);
                let contour = build_standard_gear_contour(p.teeth, p.module_mm);
                let base_name = format!("StandardGear_Z{}_M{}", p.teeth, p.module_mm);

                let created = create_gear(
                    &base_name,
                    &frame,
                    &contour,
                    p.extrude,
                    p.thickness_mm,
                    p.bore_diameter_mm,
                )?;
                Ok(json!({ "status": "ok", "created": created, "extruded": p.extrude }).to_string())
            }
            "spur-gear" => {
                let p: SpurGearParams = serde_json::from_str(&params_json)
                    .map_err(|e| invalid_field("params", e.to_string()))?;
                validate_common(p.teeth, p.module_mm, p.extrude, p.thickness_mm, p.bore_diameter_mm)?;

                let offset = [p.offset_x_mm, p.offset_y_mm, p.offset_z_mm];
                let frame = resolve_smart_frame(p.force_origin, &p.fallback_plane, offset);
                let prec = Precision::from_str(&p.precision);
                let contour = build_spur_gear_contour(
                    p.teeth,
                    p.module_mm,
                    p.pressure_angle_deg,
                    p.backlash_mm,
                    prec,
                );
                let base_name = format!("SpurGear_Z{}_M{}", p.teeth, p.module_mm);

                let created = create_gear(
                    &base_name,
                    &frame,
                    &contour,
                    p.extrude,
                    p.thickness_mm,
                    p.bore_diameter_mm,
                )?;
                Ok(json!({ "status": "ok", "created": created, "extruded": p.extrude }).to_string())
            }
            _ => Err(invalid_field("entry", format!("Unknown entry: {entry}"))),
        }
    }
}

fn validate_common(
    teeth: usize,
    module_mm: f64,
    extrude: bool,
    thickness: f64,
    bore: f64,
) -> Result<(), PluginError> {
    if teeth < 4 {
        return Err(invalid_field("teeth", "Teeth count must be at least 4."));
    }
    let root_diameter = module_mm * (teeth as f64 - 2.5);
    if bore >= root_diameter {
        return Err(invalid_field(
            "bore_diameter_mm",
            format!("Bore ({bore} mm) must be smaller than root diameter ({root_diameter:.2} mm)."),
        ));
    }
    if module_mm <= 0.0 || (extrude && thickness <= 0.0) {
        return Err(invalid_field("module_mm", "Module and thickness must be positive."));
    }
    Ok(())
}

export!(GearPlugin);