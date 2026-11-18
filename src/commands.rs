use crate::craft::plugin::commands;
use crate::craft::plugin::types::{CommandFailure, PluginError};
use crate::gears::iso::{IsoGearDimensions, Pt2};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub struct PlacementFrame {
    pub origin: [f64; 3],
    pub x_dir: [f64; 3],
    pub y_dir: [f64; 3],
}

fn exec(cmd: &str, params: Value) -> Result<Value, PluginError> {
    let raw = commands::run(cmd, &params.to_string())?;
    serde_json::from_str(&raw).map_err(|e| {
        PluginError::CommandFailed(CommandFailure {
            command: cmd.to_string(),
            reason: format!("Invalid JSON response: {e}"),
        })
    })
}

fn parse_vec3(v: Option<&Value>) -> Option<[f64; 3]> {
    let arr = v?.as_array()?;
    if arr.len() < 3 {
        return None;
    }
    Some([arr[0].as_f64()?, arr[1].as_f64()?, arr[2].as_f64()?])
}

fn parse_vec2(v: Option<&Value>) -> Option<[f64; 2]> {
    let arr = v?.as_array()?;
    if arr.len() < 2 {
        return None;
    }
    Some([arr[0].as_f64()?, arr[1].as_f64()?])
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: [f64; 3]) -> Option<[f64; 3]> {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-12 {
        None
    } else {
        Some([v[0] / len, v[1] / len, v[2] / len])
    }
}

fn basis_from_normal(n: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let Some(z) = normalize(n) else {
        return ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    };
    let ref_y = if z[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let x = normalize(cross(ref_y, z)).unwrap_or([1.0, 0.0, 0.0]);
    let y = normalize(cross(z, x)).unwrap_or([0.0, 1.0, 0.0]);
    (x, y)
}

fn project_point_onto_plane(pt: [f64; 3], plane_pt: [f64; 3], normal: [f64; 3]) -> [f64; 3] {
    let Some(n) = normalize(normal) else {
        return pt;
    };
    let diff = [pt[0] - plane_pt[0], pt[1] - plane_pt[1], pt[2] - plane_pt[2]];
    let dist = dot(diff, n);
    [pt[0] - dist * n[0], pt[1] - dist * n[1], pt[2] - dist * n[2]]
}

fn default_frame(plane: &str) -> PlacementFrame {
    match plane {
        "XZ" => PlacementFrame {
            origin: [0.0, 0.0, 0.0],
            x_dir: [1.0, 0.0, 0.0],
            y_dir: [0.0, 0.0, 1.0],
        },
        "YZ" => PlacementFrame {
            origin: [0.0, 0.0, 0.0],
            x_dir: [0.0, 1.0, 0.0],
            y_dir: [0.0, 0.0, 1.0],
        },
        _ => PlacementFrame {
            origin: [0.0, 0.0, 0.0],
            x_dir: [1.0, 0.0, 0.0],
            y_dir: [0.0, 1.0, 0.0],
        },
    }
}

fn extract_frame_object(obj: &Value) -> Option<PlacementFrame> {
    if let Some(s) = obj.as_str() {
        return Some(default_frame(s));
    }
    let origin = parse_vec3(obj.get("origin"))
        .or_else(|| parse_vec3(obj.get("point")))
        .unwrap_or([0.0, 0.0, 0.0]);

    let x_opt = parse_vec3(obj.get("x_dir"))
        .or_else(|| parse_vec3(obj.get("x_axis")))
        .or_else(|| parse_vec3(obj.get("u_dir")))
        .or_else(|| parse_vec3(obj.get("x")));

    let y_opt = parse_vec3(obj.get("y_dir"))
        .or_else(|| parse_vec3(obj.get("y_axis")))
        .or_else(|| parse_vec3(obj.get("v_dir")))
        .or_else(|| parse_vec3(obj.get("y")));

    if let (Some(x_raw), Some(y_raw)) = (x_opt, y_opt) && let (Some(x_dir), Some(y_dir)) = (normalize(x_raw), normalize(y_raw)) {
        return Some(PlacementFrame { origin, x_dir, y_dir });
    }

    let n_opt = parse_vec3(obj.get("normal"))
        .or_else(|| parse_vec3(obj.get("z_dir")))
        .or_else(|| parse_vec3(obj.get("z_axis")))
        .or_else(|| parse_vec3(obj.get("z")));

    if let Some(normal) = n_opt {
        let (x_dir, y_dir) = basis_from_normal(normal);
        return Some(PlacementFrame { origin, x_dir, y_dir });
    }
    None
}

fn frame_from_sketch_info(info: &Value) -> Option<PlacementFrame> {
    if let Some(f) = info.get("frame").and_then(extract_frame_object) {
        return Some(f);
    }
    if let Some(p) = info.get("plane").and_then(extract_frame_object) {
        return Some(p);
    }
    extract_frame_object(info)
}

fn next_unique_name(base: &str) -> String {
    let Ok(doc) = exec(
        "document.inspect",
        json!({
            "timeline": "summary",
            "bodies": "names"
        }),
    ) else {
        return format!("{base}_1");
    };

    let raw_dump = doc.to_string();
    let mut idx = 1usize;
    loop {
        let candidate = format!("{base}_{idx}");
        if !raw_dump.contains(&candidate) {
            return candidate;
        }
        idx += 1;
    }
}

pub fn resolve_smart_frame(
    force_origin: bool,
    fallback_plane: &str,
    offset: [f64; 3],
) -> PlacementFrame {
    let mut frame = if force_origin {
        default_frame(fallback_plane)
    } else {
        detect_from_selection(fallback_plane).unwrap_or_else(|| default_frame(fallback_plane))
    };

    frame.origin[0] += offset[0];
    frame.origin[1] += offset[1];
    frame.origin[2] += offset[2];
    frame
}

fn detect_from_selection(fallback_plane: &str) -> Option<PlacementFrame> {
    let sel = exec("selection.get", json!({ "detail": true })).ok()?;
    let items = sel.get("items")?.as_array()?;
    if items.is_empty() {
        return None;
    }

    if let Some(sp) = items.iter().find(|it| {
        matches!(
            it.get("kind").or_else(|| it.get("type")).and_then(Value::as_str),
            Some("sketch_point" | "sketch_midpoint")
        )
    }) {
        let sketch_id = sp.get("sketch").cloned().unwrap_or(Value::Null);
        let sk_info = if sketch_id.is_null() {
            exec("sketch.inspect", json!({})).ok()?
        } else {
            exec("sketch.inspect", json!({ "sketch": sketch_id })).ok()?
        };

        let base = frame_from_sketch_info(&sk_info).unwrap_or_else(|| default_frame(fallback_plane));

        let world_origin = if let Some(w3) = parse_vec3(sp.get("world_position"))
            .or_else(|| parse_vec3(sp.get("world")))
            .or_else(|| parse_vec3(sp.get("position")))
        {
            w3
        } else {
            let pos2 = parse_vec2(sp.get("position")).or_else(|| parse_vec2(sp.get("at")))?;
            [
                base.origin[0] + pos2[0] * base.x_dir[0] + pos2[1] * base.y_dir[0],
                base.origin[1] + pos2[0] * base.x_dir[1] + pos2[1] * base.y_dir[1],
                base.origin[2] + pos2[0] * base.x_dir[2] + pos2[1] * base.y_dir[2],
            ]
        };

        return Some(PlacementFrame {
            origin: world_origin,
            x_dir: base.x_dir,
            y_dir: base.y_dir,
        });
    }

    let mut point_3d: Option<[f64; 3]> = None;
    let mut face_pick_point: Option<[f64; 3]> = None;
    let mut plane_origin: Option<[f64; 3]> = None;
    let mut plane_normal: Option<[f64; 3]> = None;
    let mut explicit_frame: Option<PlacementFrame> = None;

    for it in items {
        let kind = it
            .get("kind")
            .or_else(|| it.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("");

        match kind {
            "vertex" | "construction_point" => {
                point_3d = parse_vec3(it.get("point"))
                    .or_else(|| parse_vec3(it.get("position")))
                    .or_else(|| parse_vec3(it.get("at")));
            }
            "face" => {
                if let Some(surf) = it.get("surface") && surf.get("type").and_then(Value::as_str) == Some("plane") {
                    plane_normal = parse_vec3(surf.get("normal"));
                    face_pick_point = parse_vec3(it.get("point"))
                        .or_else(|| parse_vec3(it.get("centroid")));
                    plane_origin = face_pick_point
                        .or_else(|| parse_vec3(surf.get("point")))
                }
            }
            "plane" => {
                let id_str = it
                    .get("id")
                    .or_else(|| it.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if matches!(id_str, "XY" | "XZ" | "YZ") {
                    explicit_frame = Some(default_frame(id_str));
                } else {
                    plane_normal = parse_vec3(it.get("normal"));
                    plane_origin = parse_vec3(it.get("origin"))
                        .or_else(|| parse_vec3(it.get("point")));
                }
            }
            _ => {}
        }
    }

    if let Some(pt) = point_3d {
        if let Some(n) = plane_normal {
            let projected = if let Some(p0) = plane_origin {
                project_point_onto_plane(pt, p0, n)
            } else {
                pt
            };
            let (x_dir, y_dir) = basis_from_normal(n);
            return Some(PlacementFrame {
                origin: projected,
                x_dir,
                y_dir,
            });
        }
        if let Some(mut ef) = explicit_frame {
            ef.origin = pt;
            return Some(ef);
        }
        let mut df = default_frame(fallback_plane);
        df.origin = pt;
        return Some(df);
    }

    if let Some(n) = plane_normal {
        let (x_dir, y_dir) = basis_from_normal(n);
        return Some(PlacementFrame {
            origin: face_pick_point.or(plane_origin).unwrap_or([0.0, 0.0, 0.0]),
            x_dir,
            y_dir,
        });
    }

    explicit_frame
}

fn draw_gear_sketch(
    sketch_name: &str,
    frame: &PlacementFrame,
    contour: &[Pt2],
    bore_diameter_mm: f64,
    dims: &IsoGearDimensions,
    include_base_circle: bool,
) -> Result<(), PluginError> {
    exec(
        "sketch.create",
        json!({
            "name": sketch_name,
            "plane": {
                "origin": frame.origin,
                "x_dir": frame.x_dir,
                "y_dir": frame.y_dir
            }
        }),
    )?;

    exec(
        "sketch.line",
        json!({
            "points": contour,
            "closed": true,
            "connect": false
        }),
    )?;

    if bore_diameter_mm > 0.0 {
        exec(
            "sketch.circle.center",
            json!({
                "center": [0.0, 0.0],
                "diameter": bore_diameter_mm,
                "connect": false
            }),
        )?;
    }

    // Центральная точка шестерни
    exec(
        "sketch.point",
        json!({
            "point": [0.0, 0.0],
            "connect": false
        }),
    )?;

    // Вспомогательные окружности (da, d, df и db для эвольвенты)
    let mut ref_diameters = vec![
        dims.addendum_radius * 2.0,
        dims.pitch_radius * 2.0,
        dims.root_radius * 2.0,
    ];
    if include_base_circle {
        ref_diameters.push(dims.base_radius * 2.0);
    }

    for dia in ref_diameters {
        if dia > 0.05 {
            exec(
                "sketch.circle.center",
                json!({
                    "center": [0.0, 0.0],
                    "diameter": dia,
                    "construction": true,
                    "connect": false
                }),
            )?;
        }
    }

    exec("sketch.finish", json!({}))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn create_extruded_gear(
    base_name: &str,
    frame: &PlacementFrame,
    dims: &IsoGearDimensions,
    contour: &[Pt2],
    extrude: bool,
    thickness_mm: f64,
    bore_diameter_mm: f64,
    include_base_circle: bool,
) -> Result<String, PluginError> {
    let unique_name = next_unique_name(base_name);
    let sketch_name = format!("{unique_name}_Sketch");

    draw_gear_sketch(
        &sketch_name,
        frame,
        contour,
        bore_diameter_mm,
        dims,
        include_base_circle,
    )?;

    if extrude {
        exec(
            "solid.extrude",
            json!({
                "sketch": sketch_name,
                "distance": thickness_mm,
                "direction": "positive",
                "operation": "new",
                "body_name": unique_name
            }),
        )?;
    }

    Ok(unique_name)
}