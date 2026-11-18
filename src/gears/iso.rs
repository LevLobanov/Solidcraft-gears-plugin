use std::f64::consts::PI;

pub type Pt2 = [f64; 2];

pub const ISO_54_MODULES: &[f64] = &[
    0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.125, 1.25, 1.375, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0, 3.5,
    4.0, 4.5, 5.0, 5.5, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0,
];

pub const ISO_53_ADDENDUM_COEFF: f64 = 1.0;
pub const ISO_53_CLEARANCE_COEFF: f64 = 0.25;
pub const ISO_53_DEDENDUM_COEFF: f64 = ISO_53_ADDENDUM_COEFF + ISO_53_CLEARANCE_COEFF;

#[derive(Debug, Clone, Copy)]
pub enum Precision {
    Low,
    Medium,
    High,
    Ultra,
}

impl Precision {
    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
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

pub fn parse_iso_module(raw: &str) -> Result<f64, String> {
    let val: f64 = raw
        .trim()
        .parse()
        .map_err(|_| format!("Invalid ISO module value: `{raw}`"))?;
    if ISO_54_MODULES.iter().any(|&m| (m - val).abs() < 1e-6) {
        Ok(val)
    } else {
        Err(format!("Module {val} mm is not in the ISO 54 standard series."))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IsoGearDimensions {
    pub teeth: usize,
    pub normal_module: f64,
    pub transverse_module: f64,
    pub normal_pressure_angle_rad: f64,
    pub transverse_pressure_angle_rad: f64,
    pub helix_angle_rad: f64,
    pub profile_shift: f64,
    pub pitch_radius: f64,
    pub base_radius: f64,
    pub addendum_radius: f64,
    pub root_radius: f64,
}

impl IsoGearDimensions {
    pub fn compute(
        teeth: usize,
        normal_module: f64,
        normal_pressure_angle_deg: f64,
        helix_angle_deg: f64,
        profile_shift: f64,
    ) -> Self {
        let z = teeth as f64;
        let beta = helix_angle_deg.to_radians();
        let alpha_n = normal_pressure_angle_deg.to_radians();

        let cos_beta = beta.cos().max(1e-6);
        let mt = normal_module / cos_beta;
        let alpha_t = (alpha_n.tan() / cos_beta).atan();

        let r_pitch = (mt * z) / 2.0;
        let r_base = r_pitch * alpha_t.cos();

        let h_a = normal_module * (ISO_53_ADDENDUM_COEFF + profile_shift);
        let h_f = normal_module * (ISO_53_DEDENDUM_COEFF - profile_shift);

        let r_addendum = r_pitch + h_a;
        let r_root = (r_pitch - h_f).max(0.2);

        Self {
            teeth,
            normal_module,
            transverse_module: mt,
            normal_pressure_angle_rad: alpha_n,
            transverse_pressure_angle_rad: alpha_t,
            helix_angle_rad: beta,
            profile_shift,
            pitch_radius: r_pitch,
            base_radius: r_base,
            addendum_radius: r_addendum,
            root_radius: r_root,
        }
    }

    /// Осевой угол закрутки (в радианах) на высоте `height_mm` по ISO 21771
    pub fn twist_angle_rad(&self, height_mm: f64, right_hand: bool) -> f64 {
        let psi = (height_mm * self.helix_angle_rad.tan()) / self.pitch_radius;
        if right_hand { psi } else { -psi }
    }

    pub fn twist_angle_deg(&self, height_mm: f64, right_hand: bool) -> f64 {
        self.twist_angle_rad(height_mm, right_hand) * (180.0 / PI)
    }
}

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

#[inline]
pub fn polar(r: f64, angle: f64) -> Pt2 {
    [r * angle.cos(), r * angle.sin()]
}