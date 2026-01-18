use garde::Validate;
use serde::Deserialize;
use serde_saphyr::{Error, Options};

fn default_offsets() -> Vec<f64> {
    vec![0.0; 6]
}
fn default_sign_corrections() -> Vec<i8> {
    vec![1; 6]
}

fn opw_geometry_parameter(v: &f64, _ctx: &()) -> garde::Result {
    if !v.is_finite() {
        return Err(garde::Error::new("must be finite"));
    }
    Ok(())
}

fn joint_offset(v: &f64, _ctx: &()) -> garde::Result {
    if !v.is_finite() {
        return Err(garde::Error::new("must be finite"));
    }
    let limit = 2.0 * std::f64::consts::PI;
    if *v < -limit || *v > limit {
        return Err(garde::Error::new("must be within [-2*PI, 2*PI]"));
    }
    Ok(())
}

fn sign_correction(v: &i8, _ctx: &()) -> garde::Result {
    if *v != -1 && *v != 1 {
        return Err(garde::Error::new("must be -1 or 1"));
    }
    Ok(())
}

#[derive(Deserialize, Validate)]
struct GeometricParameters {
    #[garde(custom(opw_geometry_parameter))]
    pub a1: f64,
    #[garde(custom(opw_geometry_parameter))]
    pub a2: f64,
    #[garde(custom(opw_geometry_parameter))]
    pub b: f64,
    #[garde(custom(opw_geometry_parameter))]
    pub c1: f64,
    #[garde(custom(opw_geometry_parameter))]
    pub c2: f64,
    #[garde(custom(opw_geometry_parameter))]
    pub c3: f64,
    #[garde(custom(opw_geometry_parameter))]
    pub c4: f64,
    /// Optional here; top-level `dof` overrides if also present
    #[serde(default)]
    #[garde(range(min = 5, max = 6))]
    pub dof: Option<i8>,
}

#[derive(Deserialize, Validate)]
struct Root {
    #[garde(dive)]
    pub opw_kinematics_geometric_parameters: GeometricParameters,
    #[serde(default = "default_offsets")]
    #[garde(length(min = 5, max = 6), inner(custom(joint_offset)))]
    pub opw_kinematics_joint_offsets: Vec<f64>,
    #[serde(default = "default_sign_corrections")]
    #[garde(length(min = 5, max = 6), inner(custom(sign_correction)))]
    pub opw_kinematics_joint_sign_corrections: Vec<i8>,
    /// Optional; overrides opw_kinematics_geometric_parameters.dof if present
    #[serde(default)]
    #[garde(range(min = 5, max = 6))]
    pub dof: Option<i8>,
}

#[test]
fn test_nan_in_conf() {
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
        contents,
        Options {
            angle_conversions: true,
            ..Default::default()
        },
    );
    match root {
        Ok(_root) => {
            panic!("Validation must fail - a1 is nan")
        }
        Err(err) => {
            assert!(err.to_string().contains(
                "^ validation error: must be finite for `opw_kinematics_geometric_parameters.a1`"),
                    "Expected substring not found: {}", err);
        }
    }
}

#[test]
fn test_invalid_sign_correction() {
    let contents = r#"
opw_kinematics_geometric_parameters:
    a1: 0.15
    a2: -0.10
    b: 0.0
    c1: 0.525
    c2: 0.77
    c3: 0.74
    c4: 0.10
opw_kinematics_joint_offsets: [0.0, 0.0, deg(-90.0), 0.0, 0.0, deg(180.0)]
opw_kinematics_joint_sign_corrections: [1, 1, 0, -1, -1, -1]
dof: 6    "#;

    let root: Result<Root, Error> = serde_saphyr::from_str_with_options_valid(
        contents,
        Options {
            angle_conversions: true,
            ..Default::default()
        },
    );
    match root {
        Ok(_root) => {
            panic!("Validation must fail - sign correction is invalid")
        }
        Err(err) => {
            assert!(err.to_string().contains(
                "^ validation error: must be -1 or 1 for `opw_kinematics_joint_sign_corrections[2]`"),
                    "Expected substring not found: {}", err);
        }
    }
}
