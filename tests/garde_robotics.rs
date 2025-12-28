//! Supports extracting OPW parameters from YAML file (optional)

use garde::Validate;
use serde::Deserialize;
use serde_saphyr::{Error, Options};

fn default_offsets() -> Vec<f64> {
    vec![0.0; 6]
}
fn default_sign_corrections() -> Vec<i8> {
    vec![1; 6]
}

fn validate_finite_f64(v: &f64, _ctx: &()) -> garde::Result {
    if !v.is_finite() {
        return Err(garde::Error::new("must be finite"));
    }
    Ok(())
}

fn validate_offset_f64(v: &f64, _ctx: &()) -> garde::Result {
    if !v.is_finite() {
        return Err(garde::Error::new("must be finite"));
    }
    let limit = 2.0 * std::f64::consts::PI;
    if *v < -limit || *v > limit {
        return Err(garde::Error::new("must be within [-2*PI, 2*PI]"));
    }
    Ok(())
}

fn validate_sign_correction_i8(v: &i8, _ctx: &()) -> garde::Result {
    if *v != -1 && *v != 1 {
        return Err(garde::Error::new("must be -1 or 1"));
    }
    Ok(())
}

#[derive(Deserialize, Validate)]
struct GeometricParameters {
    #[garde(custom(validate_finite_f64))]
    pub a1: f64,
    #[garde(custom(validate_finite_f64))]
    pub a2: f64,
    #[garde(custom(validate_finite_f64))]
    pub b: f64,
    #[garde(custom(validate_finite_f64))]
    pub c1: f64,
    #[garde(custom(validate_finite_f64))]
    pub c2: f64,
    #[garde(custom(validate_finite_f64))]
    pub c3: f64,
    #[garde(custom(validate_finite_f64))]
    pub c4: f64,
    /// Optional here; top-level `dof` overrides if also present
    #[serde(default)]
    #[garde(range(min = 5, max = 6))]
    pub dof: Option<i8>,
}

#[derive(Deserialize, Validate)]
struct Root {
    #[serde(rename = "opw_kinematics_geometric_parameters")]
    #[garde(dive)]
    pub gp: GeometricParameters,
    #[serde(default = "default_offsets", rename = "opw_kinematics_joint_offsets")]
    #[garde(length(min = 5, max = 6), inner(custom(validate_offset_f64)))]
    pub offsets: Vec<f64>,
    #[serde(
        default = "default_sign_corrections",
        rename = "opw_kinematics_joint_sign_corrections"
    )]
    #[garde(length(min = 5, max = 6), inner(custom(validate_sign_correction_i8)))]
    pub sign_corrections: Vec<i8>,
    /// Optional; overrides gp.dof if present
    #[serde(default)]
    #[garde(range(min = 5, max = 6))]
    pub dof: Option<i8>,
}

#[test]
fn test_opw_conf() {
    let contents = r#"
opw_kinematics_geometric_parameters:
  a1: .nan
  a2: -0.10
  b: 0.0
  c1: 0.525
  c2: 0.77
  c3: 0.74
  c4: 0.10
opw_kinematics_joint_offsets: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
opw_kinematics_joint_sign_corrections: [1, 1, -1, -1, -1, -1]
dof: 6
    "#;

    let root: Result<Root, Error> = serde_saphyr::from_str_with_options_valid(
        &contents,
        Options {
            angle_conversions: true,
            ..Default::default()
        },
    );
    match root {
        Ok(root) => {
            assert!(false, "Validation must fail - a1 is nan")
        }
        Err(err) => {
            println!("Error: {}", err);
        }
    }
}
