// ============================================================================
// [LEGACY / СТАБИЛЬНАЯ ОСНОВА]
// Адаптер команд SolidCraft (выделение, правый базис грани, построение эскиза
// и экструд). Менять только при изменении API SolidCraft или если для нового
// типа шестерни (например, 3D-операции) без правки здесь не обойтись.
// ============================================================================

use serde_json::{json, Value};

use crate::gears::{GearProfile2D, Pt2, Vec3};

#[derive(Debug, Clone)]
pub struct CadError {
    pub command: String,
    pub reason: String,
}

pub trait CadBackend {
    fn run_command(&self, cmd: &str, args: &Value) -> Result<Value, CadError>;
    fn log_info(&self, _msg: &str) {}
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame3 {
    pub origin: Vec3,
    pub x_dir: Vec3,
    pub y_dir: Vec3,
}

impl Frame3 {
    pub fn from_standard_plane(plane: &str) -> Self {
        match plane {
            "XZ" => Self { origin: [0.0; 3], x_dir: [1.0, 0.0, 0.0], y_dir: [0.0, 0.0, 1.0] },
            "YZ" => Self { origin: [0.0; 3], x_dir: [0.0, 1.0, 0.0], y_dir: [0.0, 0.0, 1.0] },
            _ => Self { origin: [0.0; 3], x_dir: [1.0, 0.0, 0.0], y_dir: [0.0, 1.0, 0.0] },
        }
    }

    pub fn translated(&self, offset: Vec3) -> Self {
        Self {
            origin: [
                self.origin[0] + offset[0],
                self.origin[1] + offset[1],
                self.origin[2] + offset[2],
            ],
            x_dir: self.x_dir,
            y_dir: self.y_dir,
        }
    }

    pub fn local_2d_to_world(&self, uv: Pt2) -> Vec3 {
        [
            self.origin[0] + uv[0] * self.x_dir[0] + uv[1] * self.y_dir[0],
            self.origin[1] + uv[0] * self.x_dir[1] + uv[1] * self.y_dir[1],
            self.origin[2] + uv[0] * self.x_dir[2] + uv[1] * self.y_dir[2],
        ]
    }
}

fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: Vec3) -> Option<Vec3> {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    (len >= 1e-12).then_some([v[0] / len, v[1] / len, v[2] / len])
}

fn basis_from_normal(n: Vec3) -> (Vec3, Vec3) {
    let Some(z) = normalize(n) else {
        return ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    };
    let ref_y = if z[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0] };
    let x = normalize(cross(ref_y, z)).unwrap_or([1.0, 0.0, 0.0]);
    let y = normalize(cross(z, x)).unwrap_or([0.0, 1.0, 0.0]);
    (x, y)
}

fn project_point_onto_plane(pt: Vec3, plane_pt: Vec3, normal: Vec3) -> Vec3 {
    let Some(n) = normalize(normal) else { return pt };
    let diff = [pt[0] - plane_pt[0], pt[1] - plane_pt[1], pt[2] - plane_pt[2]];
    let dist = dot(diff, n);
    [pt[0] - dist * n[0], pt[1] - dist * n[1], pt[2] - dist * n[2]]
}

fn parse_vec3(v: Option<&Value>) -> Option<Vec3> {
    let a = v?.as_array()?;
    (a.len() >= 3).then(|| Some([a[0].as_f64()?, a[1].as_f64()?, a[2].as_f64()?])).flatten()
}

fn parse_vec2(v: Option<&Value>) -> Option<Pt2> {
    let a = v?.as_array()?;
    (a.len() >= 2).then(|| Some([a[0].as_f64()?, a[1].as_f64()?])).flatten()
}

fn extract_frame_object(obj: &Value) -> Option<Frame3> {
    if let Some(s) = obj.as_str() {
        return Some(Frame3::from_standard_plane(s));
    }
    let origin = parse_vec3(obj.get("origin"))
        .or_else(|| parse_vec3(obj.get("point")))
        .unwrap_or([0.0; 3]);

    let x_opt = parse_vec3(obj.get("x_dir"))
        .or_else(|| parse_vec3(obj.get("x_axis")))
        .or_else(|| parse_vec3(obj.get("u_dir")))
        .or_else(|| parse_vec3(obj.get("x")));
    let y_opt = parse_vec3(obj.get("y_dir"))
        .or_else(|| parse_vec3(obj.get("y_axis")))
        .or_else(|| parse_vec3(obj.get("v_dir")))
        .or_else(|| parse_vec3(obj.get("y")));

    if let (Some(xr), Some(yr)) = (x_opt, y_opt) {
        if let (Some(x_dir), Some(y_dir)) = (normalize(xr), normalize(yr)) {
            return Some(Frame3 { origin, x_dir, y_dir });
        }
    }

    let n_opt = parse_vec3(obj.get("normal"))
        .or_else(|| parse_vec3(obj.get("z_dir")))
        .or_else(|| parse_vec3(obj.get("z_axis")))
        .or_else(|| parse_vec3(obj.get("z")));

    n_opt.map(|n| {
        let (x_dir, y_dir) = basis_from_normal(n);
        Frame3 { origin, x_dir, y_dir }
    })
}

fn next_unique_name(backend: &dyn CadBackend, base: &str) -> String {
    let Ok(doc) = backend.run_command(
        "document.inspect",
        &json!({ "timeline": "summary", "bodies": "names" }),
    ) else {
        return format!("{base}_1");
    };
    let raw = doc.to_string();
    (1usize..)
        .map(|i| format!("{base}_{i}"))
        .find(|name| !raw.contains(name))
        .unwrap()
}

pub fn resolve_smart_frame(
    backend: &dyn CadBackend,
    force_origin: bool,
    fallback_plane: &str,
    offset: Vec3,
) -> Frame3 {
    let base = if force_origin {
        Frame3::from_standard_plane(fallback_plane)
    } else {
        detect_from_selection(backend, fallback_plane)
            .unwrap_or_else(|| Frame3::from_standard_plane(fallback_plane))
    };
    base.translated(offset)
}

fn detect_from_selection(backend: &dyn CadBackend, fallback_plane: &str) -> Option<Frame3> {
    let sel = backend.run_command("selection.get", &json!({ "detail": true })).ok()?;
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
        let sk_args = if sketch_id.is_null() { json!({}) } else { json!({ "sketch": sketch_id }) };
        let sk_info = backend.run_command("sketch.inspect", &sk_args).ok()?;
        let base = sk_info
            .get("frame")
            .and_then(extract_frame_object)
            .or_else(|| sk_info.get("plane").and_then(extract_frame_object))
            .or_else(|| extract_frame_object(&sk_info))
            .unwrap_or_else(|| Frame3::from_standard_plane(fallback_plane));

        let world_origin = parse_vec3(sp.get("world_position"))
            .or_else(|| parse_vec3(sp.get("world")))
            .or_else(|| parse_vec3(sp.get("position")))
            .or_else(|| {
                let uv = parse_vec2(sp.get("position")).or_else(|| parse_vec2(sp.get("at")))?;
                Some(base.local_2d_to_world(uv))
            })?;

        return Some(Frame3 { origin: world_origin, x_dir: base.x_dir, y_dir: base.y_dir });
    }

    let (mut point_3d, mut face_pick, mut plane_origin, mut plane_normal, mut explicit) =
        (None, None, None, None, None);

    for it in items {
        let kind = it.get("kind").or_else(|| it.get("type")).and_then(Value::as_str).unwrap_or("");
        match kind {
            "vertex" | "construction_point" => {
                point_3d = parse_vec3(it.get("point"))
                    .or_else(|| parse_vec3(it.get("position")))
                    .or_else(|| parse_vec3(it.get("at")));
            }
            "face" => {
                if let Some(surf) = it.get("surface") {
                    if surf.get("type").and_then(Value::as_str) == Some("plane") {
                        plane_normal = parse_vec3(surf.get("normal"));
                        face_pick = parse_vec3(it.get("point")).or_else(|| parse_vec3(it.get("centroid")));
                        plane_origin = face_pick
                            .or_else(|| parse_vec3(surf.get("point")))
                            .or_else(|| parse_vec3(surf.get("origin")));
                    }
                }
            }
            "plane" => {
                let id = it.get("id").or_else(|| it.get("name")).and_then(Value::as_str).unwrap_or("");
                if matches!(id, "XY" | "XZ" | "YZ") {
                    explicit = Some(Frame3::from_standard_plane(id));
                } else {
                    plane_normal = parse_vec3(it.get("normal"));
                    plane_origin = parse_vec3(it.get("origin")).or_else(|| parse_vec3(it.get("point")));
                }
            }
            _ => {}
        }
    }

    if let Some(pt) = point_3d {
        if let Some(n) = plane_normal {
            let origin = plane_origin.map(|p0| project_point_onto_plane(pt, p0, n)).unwrap_or(pt);
            let (x_dir, y_dir) = basis_from_normal(n);
            return Some(Frame3 { origin, x_dir, y_dir });
        }
        if let Some(mut ef) = explicit {
            ef.origin = pt;
            return Some(ef);
        }
        let mut df = Frame3::from_standard_plane(fallback_plane);
        df.origin = pt;
        return Some(df);
    }

    if let Some(n) = plane_normal {
        let (x_dir, y_dir) = basis_from_normal(n);
        return Some(Frame3 {
            origin: face_pick.or(plane_origin).unwrap_or([0.0; 3]),
            x_dir,
            y_dir,
        });
    }

    explicit
}

pub fn build_gear_in_cad(
    backend: &dyn CadBackend,
    base_name: &str,
    frame: &Frame3,
    profile: &GearProfile2D,
    extrude: bool,
    thickness_mm: f64,
) -> Result<String, CadError> {
    let unique_name = next_unique_name(backend, base_name);
    let sketch_name = format!("{unique_name}_Sketch");

    backend.run_command(
        "sketch.create",
        &json!({
            "name": sketch_name,
            "plane": { "origin": frame.origin, "x_dir": frame.x_dir, "y_dir": frame.y_dir }
        }),
    )?;

    backend.run_command(
        "sketch.line",
        &json!({ "points": profile.outer_contour, "closed": true, "connect": false }),
    )?;

    if let Some(bore) = profile.bore_diameter_mm {
        backend.run_command(
            "sketch.circle.center",
            &json!({ "center": profile.center, "diameter": bore, "connect": false }),
        )?;
    }

    backend.run_command("sketch.point", &json!({ "point": profile.center, "connect": false }))?;

    for &dia in &profile.reference_diameters_mm {
        if dia > 0.05 {
            backend.run_command(
                "sketch.circle.center",
                &json!({
                    "center": profile.center,
                    "diameter": dia,
                    "construction": true,
                    "connect": false
                }),
            )?;
        }
    }

    backend.run_command("sketch.finish", &json!({}))?;

    if extrude {
        backend.run_command(
            "solid.extrude",
            &json!({
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use crate::execute_json;

    struct MockBackend {
        calls: RefCell<Vec<String>>,
        doc_reply: Value,
    }

    impl CadBackend for MockBackend {
        fn run_command(&self, cmd: &str, _args: &Value) -> Result<Value, CadError> {
            self.calls.borrow_mut().push(cmd.to_string());
            match cmd {
                "document.inspect" => Ok(self.doc_reply.clone()),
                "selection.get" => Ok(json!({ "count": 0, "items": [] })),
                _ => Ok(json!({ "ok": true })),
            }
        }
    }

    #[test]
    fn builds_3d_spur_and_increments_name() {
        let mock = MockBackend {
            calls: RefCell::new(Vec::new()),
            doc_reply: json!({ "bodies": ["SpurGear_Z24_M2_0_1"] }),
        };
        let out = execute_json(
            &mock,
            "gear",
            r#"{"gear_type":"spur","teeth":24,"iso_module":"2.0","extrude":true,"thickness_mm":10.0,"bore_diameter_mm":8.0}"#,
        )
        .unwrap();
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["created"], "SpurGear_Z24_M2_0_2");
        assert!(mock.calls.borrow().contains(&"solid.extrude".to_string()));
    }

    #[test]
    fn extrude_false_builds_2d_only() {
        let mock = MockBackend {
            calls: RefCell::new(Vec::new()),
            doc_reply: json!({ "bodies": [] }),
        };
        execute_json(
            &mock,
            "gear",
            r#"{"gear_type":"standard","teeth":20,"iso_module":"2.0","extrude":false}"#,
        )
        .unwrap();
        assert!(!mock.calls.borrow().contains(&"solid.extrude".to_string()));
    }
}