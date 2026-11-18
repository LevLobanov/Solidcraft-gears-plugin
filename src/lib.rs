mod commands;
mod gears;

wit_bindgen::generate!({
    path: "wit",
    world: "command-plugin",
});

use craft::plugin::log::{self, Level};
use craft::plugin::types::{FieldFailure, PluginError};
use exports::craft::plugin::entry::Guest;
use gears::{
    iso::IsoGearDimensions,
    spur::{self, SpurGearParams},
    standard::{self, StandardGearParams},
};
use serde_json::json;

pub fn invalid_field(field: &str, reason: impl Into<String>) -> PluginError {
    PluginError::InvalidInput(FieldFailure {
        field: field.into(),
        reason: reason.into(),
    })
}

pub fn validate_dims(
    dims: &IsoGearDimensions,
    extrude: bool,
    thickness: f64,
    bore: f64,
) -> Result<(), PluginError> {
    if dims.teeth < 4 {
        return Err(invalid_field("teeth", "Teeth count must be at least 4."));
    }
    let root_diameter = dims.root_radius * 2.0;
    if bore >= root_diameter {
        return Err(invalid_field(
            "bore_diameter_mm",
            format!("Bore ({bore} mm) must be smaller than ISO root diameter ({root_diameter:.2} mm)."),
        ));
    }
    if extrude && thickness <= 0.0 {
        return Err(invalid_field("thickness_mm", "Facewidth (thickness) must be positive."));
    }
    Ok(())
}

struct GearPlugin;

impl Guest for GearPlugin {
    fn execute(entry: String, params_json: String) -> Result<String, PluginError> {
        log::log(Level::Info, &format!("Executing ISO gear entry: {entry}"));

        match entry.as_str() {
            "standard-gear" => {
                let p: StandardGearParams = serde_json::from_str(&params_json)
                    .map_err(|e| invalid_field("params", e.to_string()))?;
                let (created, m) = standard::run(p)?;
                Ok(json!({ "status": "ok", "created": created, "module_mm": m }).to_string())
            }
            "spur-gear" => {
                let p: SpurGearParams = serde_json::from_str(&params_json)
                    .map_err(|e| invalid_field("params", e.to_string()))?;
                let (created, m) = spur::run(p)?;
                Ok(json!({ "status": "ok", "created": created, "module_mm": m }).to_string())
            }
            _ => Err(invalid_field("entry", format!("Unknown entry: {entry}"))),
        }
    }
}

export!(GearPlugin);