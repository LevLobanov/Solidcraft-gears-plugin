pub mod spur;
pub mod standard;

// ============================================================================
// [LEGACY / СТАБИЛЬНАЯ ОСНОВА]
// Базовые структуры (GearSpec, IsoGearDimensions, GearProfile2D) и математика.
// Менять только при веской причине — если без правок решение для нового типа
// шестерни получается слишком громоздким, некрасивым или дублирует код.
// ============================================================================

pub type Pt2 = [f64; 2];
pub type Vec3 = [f64; 3];

/// Стандартный ряд модулей ISO 54:1996 (Ряд I + Ряд II), мм.
pub const ISO_54_MODULES: &[f64] = &[
    0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.125, 1.25, 1.375, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0, 3.5,
    4.0, 4.5, 5.0, 5.5, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0,
];

/// Коэффициенты исходного контура ISO 53:1998 (Профиль A).
pub const ISO_53_ADDENDUM_COEFF: f64 = 1.0;
pub const ISO_53_CLEARANCE_COEFF: f64 = 0.25;
pub const ISO_53_DEDENDUM_COEFF: f64 = ISO_53_ADDENDUM_COEFF + ISO_53_CLEARANCE_COEFF;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub field: &'static str,
    pub reason: String,
}

impl ValidationError {
    pub fn new(field: &'static str, reason: impl Into<String>) -> Self {
        Self { field, reason: reason.into() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GearKind {
    Standard,
    Spur,
}

impl GearKind {
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "spur" | "spur-gear" => Ok(Self::Spur),
            "standard" | "standard-gear" => Ok(Self::Standard),
            other => Err(ValidationError::new(
                "gear_type",
                format!("Unknown gear type `{other}`. Expected `spur` or `standard`."),
            )),
        }
    }

    pub fn name_prefix(self) -> &'static str {
        match self {
            Self::Standard => "StandardGear",
            Self::Spur => "SpurGear",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    Low,
    Medium,
    High,
    Ultra,
}

impl Precision {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "low" => Self::Low,
            "high" => Self::High,
            "ultra" => Self::Ultra,
            _ => Self::Medium,
        }
    }

    pub fn involute_samples(self) -> usize {
        match self {
            Self::Low => 4,
            Self::Medium => 8,
            Self::High => 16,
            Self::Ultra => 32,
        }
    }
}

pub fn parse_iso_module(raw: &str) -> Result<f64, ValidationError> {
    let val: f64 = raw.trim().parse().map_err(|_| {
        ValidationError::new("iso_module", format!("Invalid ISO module value: `{raw}`"))
    })?;
    if ISO_54_MODULES.iter().any(|&m| (m - val).abs() < 1e-6) {
        Ok(val)
    } else {
        Err(ValidationError::new(
            "iso_module",
            format!("Module {val} mm is not in the ISO 54 standard series."),
        ))
    }
}

/// Универсальная спецификация шестерни (не зависит от JSON / WIT).
#[derive(Debug, Clone)]
pub struct GearSpec {
    pub kind: GearKind,
    pub teeth: usize,
    pub module_mm: f64,
    pub module_label: String,
    pub pressure_angle_deg: f64,
    pub profile_shift: f64,
    pub backlash_mm: f64,
    pub precision: Precision,
    pub extrude: bool,
    pub thickness_mm: f64,
    pub bore_diameter_mm: f64,
}

/// Вычисленные размеры цилиндрического колеса по ISO 21771.
#[derive(Debug, Clone, Copy)]
pub struct IsoGearDimensions {
    pub teeth: usize,
    pub normal_module: f64,
    pub normal_pressure_angle_rad: f64,
    pub transverse_pressure_angle_rad: f64,
    pub profile_shift: f64,
    pub pitch_radius: f64,
    pub base_radius: f64,
    pub addendum_radius: f64,
    pub root_radius: f64,
}

impl IsoGearDimensions {
    pub fn compute(spec: &GearSpec) -> Self {
        let z = spec.teeth as f64;
        let alpha = spec.pressure_angle_deg.to_radians();
        let m = spec.module_mm;

        let r_pitch = (m * z) / 2.0;
        let r_base = r_pitch * alpha.cos();
        let h_a = m * (ISO_53_ADDENDUM_COEFF + spec.profile_shift);
        let h_f = m * (ISO_53_DEDENDUM_COEFF - spec.profile_shift);

        Self {
            teeth: spec.teeth,
            normal_module: m,
            normal_pressure_angle_rad: alpha,
            transverse_pressure_angle_rad: alpha,
            profile_shift: spec.profile_shift,
            pitch_radius: r_pitch,
            base_radius: r_base,
            addendum_radius: r_pitch + h_a,
            root_radius: (r_pitch - h_f).max(0.2),
        }
    }

    pub fn validate(&self, spec: &GearSpec) -> Result<(), ValidationError> {
        let min_teeth = match spec.kind {
            GearKind::Standard => 4,
            GearKind::Spur => 6,
        };
        if spec.teeth < min_teeth || spec.teeth > 200 {
            return Err(ValidationError::new(
                "teeth",
                format!("Teeth count must be in range {min_teeth}..=200."),
            ));
        }
        if !(14.5..=30.0).contains(&spec.pressure_angle_deg) {
            return Err(ValidationError::new(
                "pressure_angle_deg",
                "Pressure angle must be between 14.5 and 30.0 degrees.",
            ));
        }
        if spec.bore_diameter_mm < 0.0 {
            return Err(ValidationError::new("bore_diameter_mm", "Bore cannot be negative."));
        }
        let root_diameter = self.root_radius * 2.0;
        if spec.bore_diameter_mm >= root_diameter {
            return Err(ValidationError::new(
                "bore_diameter_mm",
                format!(
                    "Bore ({} mm) must be smaller than ISO root diameter ({root_diameter:.2} mm).",
                    spec.bore_diameter_mm
                ),
            ));
        }
        if spec.extrude && spec.thickness_mm <= 0.0 {
            return Err(ValidationError::new("thickness_mm", "Facewidth must be positive."));
        }
        Ok(())
    }
}

/// Готовый 2D-профиль шестерни, который возвращает любой генератор.
#[derive(Debug, Clone)]
pub struct GearProfile2D {
    pub dims: IsoGearDimensions,
    pub outer_contour: Vec<Pt2>,
    pub bore_diameter_mm: Option<f64>,
    pub reference_diameters_mm: Vec<f64>,
    pub center: Pt2,
}

#[inline]
pub fn polar(r: f64, angle: f64) -> Pt2 {
    [r * angle.cos(), r * angle.sin()]
}

/// Удаляет соседние точки ближе `min_dist` (включая замыкающий сегмент `last -> first`).
pub fn dedup_contour(raw: Vec<Pt2>, min_dist: f64) -> Vec<Pt2> {
    let min_dist_sq = min_dist * min_dist;
    let mut out: Vec<Pt2> = Vec::with_capacity(raw.len());

    for pt in raw {
        if let Some(last) = out.last() {
            let dx = pt[0] - last[0];
            let dy = pt[1] - last[1];
            if dx * dx + dy * dy < min_dist_sq {
                continue;
            }
        }
        out.push(pt);
    }

    while out.len() > 3 {
        let first = out[0];
        let last = *out.last().unwrap();
        let dx = first[0] - last[0];
        let dy = first[1] - last[1];
        if dx * dx + dy * dy < min_dist_sq {
            out.pop();
        } else {
            break;
        }
    }

    out
}

/// Диспетчер генераторов шестерён.
pub fn generate_gear_profile(spec: &GearSpec) -> Result<GearProfile2D, ValidationError> {
    match spec.kind {
        GearKind::Standard => standard::generate(spec),
        GearKind::Spur => spur::generate(spec),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_54_and_21771_checks() {
        assert_eq!(parse_iso_module("2.0").unwrap(), 2.0);
        assert!(parse_iso_module("2.37").is_err());

        let spec = GearSpec {
            kind: GearKind::Spur,
            teeth: 24,
            module_mm: 2.0,
            module_label: "2.0".into(),
            pressure_angle_deg: 20.0,
            profile_shift: 0.0,
            backlash_mm: 0.04,
            precision: Precision::Medium,
            extrude: true,
            thickness_mm: 10.0,
            bore_diameter_mm: 8.0,
        };
        let d = IsoGearDimensions::compute(&spec);
        assert!((d.pitch_radius * 2.0 - 48.0).abs() < 1e-9);
        assert!((d.addendum_radius * 2.0 - 52.0).abs() < 1e-9);
        assert!((d.root_radius * 2.0 - 43.0).abs() < 1e-9);
        assert!(d.validate(&spec).is_ok());
    }
}