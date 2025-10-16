#[cfg(test)]
#[cfg(feature = "robotics")]
mod tests {
    use core::f64::consts::PI;
    use serde::Deserialize;
    use serde_saphyr::{from_str_with_options, Options};

    #[derive(Debug, Deserialize)]
    struct RoboFloats {
        // Plain and tagged numbers
        plain: f64,
        rad_tag: f64,     // !radians 0.15 => 0.15
        deg_tag: f64,     // !degrees 180 => PI

        // Constants and expressions
        pi_const: f64,
        tau_const: f64,
        expr_mul: f64,    // 2*pi
        expr_div: f64,    // pi/2
        expr_complex: f64,// 1 + 2*(3 - 4/5)

        // YAML special floats
        inf1: f64,
        inf2: f64,
        ninf: f64,
        nan1: f64,

        // Functions (explicit units)
        func_deg: f64,    // deg(180) => PI
        func_rad: f64,    // rad(pi) => PI

        // Quoted functions should also be parsed (typed target is f64)
        quoted_deg: f64,
        quoted_rad: f64,

        // Also a couple of f32 targets to ensure both sizes work
        f32_from_deg: f32,
        f32_plain: f32,
    }

    #[test]
    fn robotics_angles_end_to_end() {
        // YAML with various robotics-style angle forms and expressions
        let yaml = r#"
plain: 0.15
rad_tag: !radians 0.15
deg_tag: !degrees 180
pi_const: pi
tau_const: TAU
expr_mul: 2*pi
expr_div: pi/2
expr_complex: 1 + 2*(3 - 4/5)
inf1: .inf
inf2: +.Inf
ninf: -.INF
nan1: .NaN
func_deg: deg(180)
func_rad: rad(pi)
quoted_deg: "deg(90)"
quoted_rad: 'rad(pi/2)'
f32_from_deg: deg(90)
f32_plain: 1.25
"#;

        let options = Options {
            angle_conversions: true, // enable robotics angle parsing
            .. Options::default()
        };

        let v: RoboFloats = from_str_with_options(yaml, options).expect("parse robotics YAML");

        // Basic numbers and tags
        assert!((v.plain - 0.15).abs() < 1e-12);
        assert!((v.rad_tag - 0.15).abs() < 1e-12);
        assert!((v.deg_tag - PI).abs() < 1e-12);

        // Constants and expressions
        assert!((v.pi_const - PI).abs() < 1e-12);
        assert!((v.tau_const - 2.0 * PI).abs() < 1e-12);
        assert!((v.expr_mul - 2.0 * PI).abs() < 1e-12);
        assert!((v.expr_div - (PI / 2.0)).abs() < 1e-12);
        assert!((v.expr_complex - 5.4).abs() < 1e-12);

        // Specials
        assert!(v.inf1.is_infinite() && v.inf1.is_sign_positive());
        assert!(v.inf2.is_infinite() && v.inf2.is_sign_positive());
        assert!(v.ninf.is_infinite() && v.ninf.is_sign_negative());
        assert!(v.nan1.is_nan());

        // Functions
        assert!((v.func_deg - PI).abs() < 1e-12);
        assert!((v.func_rad - PI).abs() < 1e-12);

        // Quoted forms should be parsed the same since the target is f64
        assert!((v.quoted_deg - (PI / 2.0)).abs() < 1e-12); // deg(90) = PI/2
        assert!((v.quoted_rad - (PI / 2.0)).abs() < 1e-12); // rad(pi/2) = PI/2

        // f32s
        assert!((v.f32_from_deg as f64 - (PI / 2.0)).abs() < 1e-6 as f64);
        assert!((v.f32_plain as f64 - 1.25).abs() < 1e-6 as f64);
    }
}
