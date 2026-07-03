#![cfg(all(feature = "serialize", feature = "deserialize"))]

use serde::{Deserialize, Serialize};

mod zmij_format_tests {
    use super::*;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Wrapper {
        v: f64,
    }

    fn round_trip(val: f64) -> String {
        let w = Wrapper { v: val };
        serde_saphyr::to_string(&w).unwrap()
    }

    #[test]
    fn nan() {
        let s = round_trip(f64::NAN);
        assert!(s.contains(".nan"), "expected .nan, got: {s}");
    }

    #[test]
    fn positive_inf() {
        let s = round_trip(f64::INFINITY);
        assert!(s.contains(".inf"), "expected .inf, got: {s}");
    }

    #[test]
    fn negative_inf() {
        let s = round_trip(f64::NEG_INFINITY);
        assert!(s.contains("-.inf"), "expected -.inf, got: {s}");
    }

    #[test]
    fn zero() {
        let s = round_trip(0.0);
        // Must contain a decimal point
        assert!(s.contains('.'), "expected decimal point, got: {s}");
    }

    #[test]
    fn small_exponent() {
        // 4e-6 should become 4.0e-6 (decimal point inserted)
        let s = round_trip(4e-6);
        assert!(s.contains('.'), "expected decimal point, got: {s}");
    }

    #[test]
    fn large_exponent() {
        let s = round_trip(1e20);
        // Should have exponent sign
        assert!(
            s.contains("e+")
                || s.contains("e-")
                || s.contains("E+")
                || s.contains("E-")
                || s.contains('.'),
            "expected proper float format, got: {s}"
        );
    }

    #[test]
    fn regular_float() {
        let s = round_trip(std::f64::consts::PI);
        assert!(s.contains("3.14159"), "expected PI (~3.14159), got: {s}");
    }

    #[test]
    fn integer_like_float() {
        // 1.0 should keep decimal point
        let s = round_trip(1.0);
        assert!(s.contains('.'), "expected decimal point for 1.0, got: {s}");
    }

    #[test]
    fn f32_nan() {
        #[derive(Serialize)]
        struct W32 {
            v: f32,
        }
        let s = serde_saphyr::to_string(&W32 { v: f32::NAN }).unwrap();
        assert!(s.contains(".nan"));
    }

    #[test]
    fn f32_inf() {
        #[derive(Serialize)]
        struct W32 {
            v: f32,
        }
        let s = serde_saphyr::to_string(&W32 { v: f32::INFINITY }).unwrap();
        assert!(s.contains(".inf"));
    }

    #[test]
    fn f32_neg_inf() {
        #[derive(Serialize)]
        struct W32 {
            v: f32,
        }
        let s = serde_saphyr::to_string(&W32 {
            v: f32::NEG_INFINITY,
        })
        .unwrap();
        assert!(s.contains("-.inf"));
    }

    /// Exercise the write_float_string path via to_writer (fmt::Write)
    #[test]
    fn write_path_nan() {
        #[derive(Serialize)]
        struct W {
            v: f64,
        }
        let mut buf = String::new();
        serde_saphyr::to_fmt_writer(&mut buf, &W { v: f64::NAN }).unwrap();
        assert!(buf.contains(".nan"));
    }

    #[test]
    fn write_path_inf() {
        let mut buf = String::new();
        #[derive(Serialize)]
        struct W {
            v: f64,
        }
        serde_saphyr::to_fmt_writer(&mut buf, &W { v: f64::INFINITY }).unwrap();
        assert!(buf.contains(".inf"));
    }

    #[test]
    fn write_path_neg_inf() {
        let mut buf = String::new();
        #[derive(Serialize)]
        struct W {
            v: f64,
        }
        serde_saphyr::to_fmt_writer(
            &mut buf,
            &W {
                v: f64::NEG_INFINITY,
            },
        )
        .unwrap();
        assert!(buf.contains("-.inf"));
    }

    #[test]
    fn write_path_small_exponent() {
        let mut buf = String::new();
        #[derive(Serialize)]
        struct W {
            v: f64,
        }
        serde_saphyr::to_fmt_writer(&mut buf, &W { v: 4e-6 }).unwrap();
        assert!(buf.contains('.'), "expected decimal point, got: {buf}");
    }

    #[test]
    fn write_path_integer_like() {
        let mut buf = String::new();
        #[derive(Serialize)]
        struct W {
            v: f64,
        }
        serde_saphyr::to_fmt_writer(&mut buf, &W { v: 1.0 }).unwrap();
        assert!(buf.contains('.'), "expected decimal point, got: {buf}");
    }

    #[test]
    fn write_path_large_exponent() {
        let mut buf = String::new();
        #[derive(Serialize)]
        struct W {
            v: f64,
        }
        serde_saphyr::to_fmt_writer(&mut buf, &W { v: 1e20 }).unwrap();
        assert!(buf.contains("e+"), "expected e+ exponent sign, got: {buf}");
    }

    #[test]
    fn write_path_scientific_decimal_pos() {
        let mut buf = String::new();
        #[derive(Serialize)]
        struct W {
            v: f64,
        }
        serde_saphyr::to_fmt_writer(&mut buf, &W { v: 1.23e20 }).unwrap();
        assert!(
            buf.contains("e+"),
            "expected e+ exponent sign with decimal mantissa, got: {buf}"
        );
    }

    #[test]
    fn write_path_scientific_decimal_neg() {
        let mut buf = String::new();
        #[derive(Serialize)]
        struct W {
            v: f64,
        }
        serde_saphyr::to_fmt_writer(&mut buf, &W { v: 1.23e-10 }).unwrap();
        assert!(buf.contains("e-"), "expected e- exponent sign, got: {buf}");
    }

    #[test]
    fn write_path_f32_large_exp() {
        let mut buf = String::new();
        #[derive(Serialize)]
        struct W {
            v: f32,
        }
        serde_saphyr::to_fmt_writer(&mut buf, &W { v: 1e20f32 }).unwrap();
        assert!(
            buf.contains("e"),
            "expected scientific notation, got: {buf}"
        );
    }

    #[test]
    fn float_map_keys() {
        use serde::Serializer;
        use std::collections::HashMap;
        use std::hash::{Hash, Hasher};

        struct DummyF64(pub f64);

        impl PartialEq for DummyF64 {
            fn eq(&self, other: &Self) -> bool {
                self.0.to_bits() == other.0.to_bits()
            }
        }

        impl Eq for DummyF64 {}

        impl Hash for DummyF64 {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.0.to_bits().hash(state)
            }
        }

        impl serde::Serialize for DummyF64 {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_f64(self.0)
            }
        }

        let mut map: HashMap<DummyF64, String> = HashMap::new();
        map.insert(DummyF64(1.0), "one".to_string());
        map.insert(DummyF64(1e6), "million".to_string());
        map.insert(DummyF64(4e-6), "small".to_string());
        let yaml = serde_saphyr::to_string(&map).unwrap();
        assert!(yaml.contains("one"));
        assert!(yaml.contains("million"));
        assert!(yaml.contains("small"));
        // Covers push_float_string via KeyScalarSink for float keys
    }

    #[test]
    fn round_trip_scientific_decimal_pos() {
        let s = round_trip(1.23e20);
        assert!(s.contains("e+"), "expected e+ exponent sign, got: {s}");
    }

    #[test]
    fn round_trip_scientific_decimal_neg() {
        let s = round_trip(1.23e-10);
        assert!(s.contains("e-"), "expected e- exponent sign, got: {s}");
    }
}

#[test]
fn serialize_f32_nan() {
    let s = serde_saphyr::to_string(&f32::NAN).unwrap();
    assert!(s.contains(".nan"));
}

#[test]
fn serialize_f32_inf() {
    let s = serde_saphyr::to_string(&f32::INFINITY).unwrap();
    assert!(s.contains(".inf"));
}

#[test]
fn serialize_f32_neg_inf() {
    let s = serde_saphyr::to_string(&f32::NEG_INFINITY).unwrap();
    assert!(s.contains("-.inf"));
}

#[test]
fn serialize_f64_nan() {
    let s = serde_saphyr::to_string(&f64::NAN).unwrap();
    assert!(s.contains(".nan"));
}

#[test]
fn serialize_f64_inf() {
    let s = serde_saphyr::to_string(&f64::INFINITY).unwrap();
    assert!(s.contains(".inf"));
}
