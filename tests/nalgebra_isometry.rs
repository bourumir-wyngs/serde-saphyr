use nalgebra::{Isometry3, Translation3, UnitQuaternion, Vector3};
use serde::{Deserialize, Serialize};

use serde_saphyr::{from_str, to_string};

#[derive(Debug, Serialize, Deserialize)]
struct Pose {
    pose: Isometry3<f64>,
}

fn approx_iso_eq(a: &Isometry3<f64>, b: &Isometry3<f64>, eps: f64) -> bool {
    let ta = a.translation.vector;
    let tb = b.translation.vector;
    if (ta.x - tb.x).abs() > eps || (ta.y - tb.y).abs() > eps || (ta.z - tb.z).abs() > eps {
        return false;
    }

    let a_rm = a.rotation.to_rotation_matrix();
    let b_rm = b.rotation.to_rotation_matrix();
    let ra = a_rm.matrix();
    let rb = b_rm.matrix();

    for i in 0..3 {
        for j in 0..3 {
            if (ra[(i, j)] - rb[(i, j)]).abs() > eps {
                return false;
            }
        }
    }
    true
}

#[test]
fn isometry3_roundtrip_yaml() {
    // Construct a nontrivial isometry (rotation + translation)
    let angle = std::f64::consts::FRAC_PI_3; // 60 degrees
    let axis = nalgebra::Unit::new_normalize(Vector3::new(0.1, 0.3, 0.9));
    let rot = UnitQuaternion::from_axis_angle(&axis, angle);
    let trans = Translation3::new(1.0, -2.5, 3.25);
    let iso = Isometry3::from_parts(trans, rot);

    let pose = Pose { pose: iso };

    // Serialize to YAML
    let yaml = to_string(&pose).expect("serialize Isometry3 to YAML");
    //println!("{}", yaml);

    // Deserialize back
    let decoded: Pose = from_str(&yaml).expect("deserialize Isometry3 from YAML");

    assert!(
        approx_iso_eq(&pose.pose, &decoded.pose, 1e-12),
        "Roundtrip Isometry3 mismatch. YAML was:\n{}",
        yaml
    );
}
