pub mod gears;
pub mod cad;

use serde::Deserialize;
use serde_json::json;

use cad::{build_gear_in_cad, resolve_smart_frame, CadBackend, CadError};
use gears::{generate_gear_profile, parse_iso_module, GearKind, GearSpec, Precision};

#[derive(Debug)]
pub enum AppError {
    InvalidField { field: String, reason: String },
    Cad(CadError),
}

#[derive(Deserialize)]
struct GearRequest {
    #[serde(default)]
    gear_type: Option<String>,
    teeth: usize,
    iso_module: String,
    #[serde(default = "default_pressure_angle")]
    pressure_angle_deg: f64,
    #[serde(default)]
    profile_shift: f64,
    #[serde(default = "default_backlash")]
    backlash_mm: f64,
    #[serde(default = "default_precision")]
    precision: String,
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

fn default_pressure_angle() -> f64 { 20.0 }
fn default_backlash() -> f64 { 0.04 }
fn default_precision() -> String { "medium".into() }
fn default_true() -> bool { true }
fn default_thickness() -> f64 { 10.0 }
fn default_plane() -> String { "XY".into() }

pub fn execute_json(
    backend: &dyn CadBackend,
    entry: &str,
    params_json: &str,
) -> Result<String, AppError> {
    backend.log_info(&format!("Executing gear entry: {entry}"));

    let req: GearRequest = serde_json::from_str(params_json).map_err(|e| AppError::InvalidField {
        field: "params".into(),
        reason: e.to_string(),
    })?;

    // [LEGACY] Поддержка старых имён точек входа (`standard-gear`, `spur-gear`)
    // для обратной совместимости. Не удалять без веской причины.
    let kind = match entry {
        "gear" => GearKind::parse(req.gear_type.as_deref().unwrap_or("spur")),
        "standard-gear" => Ok(GearKind::Standard),
        "spur-gear" => Ok(GearKind::Spur),
        _ => {
            return Err(AppError::InvalidField {
                field: "entry".into(),
                reason: format!("Unknown entry: {entry}"),
            })
        }
    }
    .map_err(|e| AppError::InvalidField { field: e.field.into(), reason: e.reason })?;

    let module_mm = parse_iso_module(&req.iso_module)
        .map_err(|e| AppError::InvalidField { field: e.field.into(), reason: e.reason })?;

    let spec = GearSpec {
        kind,
        teeth: req.teeth,
        module_mm,
        module_label: req.iso_module.clone(),
        pressure_angle_deg: req.pressure_angle_deg,
        profile_shift: req.profile_shift,
        backlash_mm: req.backlash_mm,
        precision: Precision::parse(&req.precision),
        extrude: req.extrude,
        thickness_mm: req.thickness_mm,
        bore_diameter_mm: req.bore_diameter_mm,
    };

    let profile = generate_gear_profile(&spec)
        .map_err(|e| AppError::InvalidField { field: e.field.into(), reason: e.reason })?;

    let offset = [req.offset_x_mm, req.offset_y_mm, req.offset_z_mm];
    let frame = resolve_smart_frame(backend, req.force_origin, &req.fallback_plane, offset);
    let base_name = format!(
        "{}_Z{}_M{}",
        spec.kind.name_prefix(),
        spec.teeth,
        spec.module_label.replace('.', "_")
    );

    let created = build_gear_in_cad(
        backend,
        &base_name,
        &frame,
        &profile,
        spec.extrude,
        spec.thickness_mm,
    )
    .map_err(AppError::Cad)?;

    Ok(json!({
        "status": "ok",
        "created": created,
        "gear_type": spec.kind.name_prefix(),
        "module_mm": spec.module_mm,
        "pitch_diameter_mm": profile.dims.pitch_radius * 2.0,
        "outside_diameter_mm": profile.dims.addendum_radius * 2.0,
        "root_diameter_mm": profile.dims.root_radius * 2.0,
        "extruded": spec.extrude
    })
    .to_string())
}

// ============================================================================
// [LEGACY / СТАБИЛЬНАЯ ОСНОВА]
// ABI-обвязка под `craft:plugin@0.1.0` (работает в SolidCraft 0.41–0.45+).
// Менять только при переходе на новую версию WIT-контракта.
// ============================================================================
#[cfg(target_arch = "wasm32")]
mod wasm_entry {
    #![allow(unsafe_op_in_unsafe_fn)]

    wit_bindgen::generate!({
        path: "wit",
        world: "command-plugin",
    });

    use craft::plugin::commands;
    use craft::plugin::log::{self, Level};
    use craft::plugin::types::{CommandFailure, FieldFailure, PluginError};
    use exports::craft::plugin::entry::Guest;
    use serde_json::Value;

    use crate::cad::{CadBackend, CadError};
    use crate::{execute_json, AppError};

    struct WitBackend;

    impl CadBackend for WitBackend {
        fn run_command(&self, cmd: &str, args: &Value) -> Result<Value, CadError> {
            let raw = commands::run(cmd, &args.to_string()).map_err(|err| CadError {
                command: cmd.to_string(),
                reason: format!("{err:?}"),
            })?;
            serde_json::from_str(&raw).map_err(|e| CadError {
                command: cmd.to_string(),
                reason: format!("Invalid JSON response: {e}"),
            })
        }

        fn log_info(&self, msg: &str) {
            log::log(Level::Info, msg);
        }
    }

    struct GearPlugin;

    impl Guest for GearPlugin {
        fn execute(entry: String, params_json: String) -> Result<String, PluginError> {
            execute_json(&WitBackend, &entry, &params_json).map_err(|err| match err {
                AppError::InvalidField { field, reason } => {
                    PluginError::InvalidInput(FieldFailure { field, reason })
                }
                AppError::Cad(CadError { command, reason }) => {
                    PluginError::CommandFailed(CommandFailure { command, reason })
                }
            })
        }
    }

    export!(GearPlugin);
}