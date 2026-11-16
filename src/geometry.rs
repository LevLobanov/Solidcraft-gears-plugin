use std::f64::consts::{PI, TAU};

pub type Pt2 = [f64; 2];

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

fn dedup_contour(raw: Vec<Pt2>, min_dist: f64) -> Vec<Pt2> {
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

pub fn build_standard_gear_contour(teeth: usize, module_mm: f64) -> Vec<Pt2> {
    let m = module_mm;
    let z = teeth as f64;
    let r_pitch = (m * z) / 2.0;
    let r_addendum = r_pitch + m;
    let r_dedendum = (r_pitch - 1.25 * m).max(0.5);

    let step = TAU / z;
    let mut pts = Vec::with_capacity(teeth * 5);

    for i in 0..teeth {
        let base = (i as f64) * step;
        pts.push(polar(r_dedendum, base));
        pts.push(polar(r_dedendum, base + step * 0.18));
        pts.push(polar(r_addendum, base + step * 0.32));
        pts.push(polar(r_addendum, base + step * 0.68));
        pts.push(polar(r_dedendum, base + step * 0.82));
    }

    dedup_contour(pts, 1e-4)
}

pub fn build_spur_gear_contour(
    teeth: usize,
    module_mm: f64,
    pressure_angle_deg: f64,
    backlash_mm: f64,
    precision: Precision,
) -> Vec<Pt2> {
    let z = teeth as f64;
    let m = module_mm;
    let alpha = pressure_angle_deg.to_radians();

    let r_pitch = (m * z) / 2.0;
    let r_base = r_pitch * alpha.cos();
    let r_addendum = r_pitch + m;
    let r_dedendum = (r_pitch - 1.25 * m).max(0.5);

    let inv_alpha = alpha.tan() - alpha;
    let half_tooth_pitch_angle = (PI / (2.0 * z)) - (backlash_mm / (2.0 * r_pitch));
    let half_tooth_base_angle = half_tooth_pitch_angle + inv_alpha;

    let t_max = ((r_addendum / r_base).powi(2) - 1.0).max(0.0).sqrt();
    let t_min = if r_dedendum > r_base {
        ((r_dedendum / r_base).powi(2) - 1.0).sqrt()
    } else {
        0.0
    };

    let samples = precision.involute_samples();
    let step = TAU / z;
    let mut pts = Vec::with_capacity(teeth * (samples * 2 + 6));

    for i in 0..teeth {
        let center_angle = (i as f64) * step;

        let root_start_angle = center_angle - step / 2.0;
        pts.push(polar(r_dedendum, root_start_angle));

        if r_dedendum < r_base {
            pts.push(polar(r_dedendum, center_angle - half_tooth_base_angle));
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

        if r_dedendum < r_base {
            pts.push(polar(r_base, center_angle + half_tooth_base_angle));
            pts.push(polar(r_dedendum, center_angle + half_tooth_base_angle));
        }
    }

    dedup_contour(pts, 1e-4)
}

#[inline]
fn polar(r: f64, angle: f64) -> Pt2 {
    [r * angle.cos(), r * angle.sin()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spur_gear_has_no_zero_length_segments() {
        for prec in [Precision::Low, Precision::Medium, Precision::High, Precision::Ultra] {
            let pts = build_spur_gear_contour(24, 2.0, 20.0, 0.05, prec);
            assert!(pts.len() > 24 * 4);
            for w in pts.windows(2) {
                let d = ((w[0][0] - w[1][0]).powi(2) + (w[0][1] - w[1][1]).powi(2)).sqrt();
                assert!(d >= 1e-4, "Zero-length segment found: {d}");
            }
        }
    }
}