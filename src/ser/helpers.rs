use serde_core::ser::{self, Serialize, Serializer};
use std::fmt::{self, Write};

use super::super::quoting::{is_plain_safe, is_plain_value_safe};
use super::super::zmij_format;
use super::super::{Error, Result};

// ------------------------------------------------------------
// Helpers used for extracting ptr/bool inside tuple payloads
// ------------------------------------------------------------

/// Minimal serializer that captures a numeric `usize` from a serialized field.
///
/// Used internally to read the raw pointer value encoded as the first field
/// of our internal anchor tuple payloads.
#[derive(Default)]
pub(super) struct UsizeCapture {
    v: Option<usize>,
}
impl Serializer for &mut UsizeCapture {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = ser::Impossible<(), Error>;
    type SerializeTuple = ser::Impossible<(), Error>;
    type SerializeTupleStruct = ser::Impossible<(), Error>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;
    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStruct = ser::Impossible<(), Error>;
    type SerializeStructVariant = ser::Impossible<(), Error>;

    fn serialize_i8(self, _v: i8) -> Result<()> {
        ptr_unsigned_err()
    }
    fn serialize_i16(self, _v: i16) -> Result<()> {
        ptr_unsigned_err()
    }
    fn serialize_i32(self, _v: i32) -> Result<()> {
        ptr_unsigned_err()
    }
    fn serialize_i64(self, _v: i64) -> Result<()> {
        ptr_unsigned_err()
    }
    fn serialize_u8(self, v: u8) -> Result<()> {
        self.v = Some(v as usize);
        Ok(())
    }
    fn serialize_u16(self, v: u16) -> Result<()> {
        self.v = Some(v as usize);
        Ok(())
    }
    fn serialize_u32(self, v: u32) -> Result<()> {
        self.v = Some(v as usize);
        Ok(())
    }
    fn serialize_u64(self, v: u64) -> Result<()> {
        self.v = Some(v as usize);
        Ok(())
    }
    fn serialize_u128(self, v: u128) -> Result<()> {
        self.v = Some(v as usize);
        Ok(())
    }
    fn serialize_f32(self, _v: f32) -> Result<()> {
        ptr_unsigned_err()
    }
    fn serialize_f64(self, _v: f64) -> Result<()> {
        ptr_unsigned_err()
    }
    fn serialize_bool(self, _v: bool) -> Result<()> {
        ptr_unsigned_err()
    }
    fn serialize_char(self, _v: char) -> Result<()> {
        Err(Error::unexpected("ptr expects number"))
    }
    fn serialize_str(self, _v: &str) -> Result<()> {
        Err(Error::unexpected("ptr expects number"))
    }
    fn serialize_bytes(self, _v: &[u8]) -> Result<()> {
        Err(Error::unexpected("ptr expects number"))
    }
    fn serialize_none(self) -> Result<()> {
        Err(Error::unexpected("ptr cannot be none"))
    }
    fn serialize_some<T: ?Sized + Serialize>(self, _value: &T) -> Result<()> {
        Err(Error::unexpected("ptr not option"))
    }
    fn serialize_unit(self) -> Result<()> {
        Err(Error::unexpected("ptr cannot be unit"))
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        unexpected_e()
    }
    fn serialize_unit_variant(self, _name: &'static str, _i: u32, _v: &'static str) -> Result<()> {
        unexpected_e()
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<()> {
        unexpected_e()
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _i: u32,
        _v: &'static str,
        _value: &T,
    ) -> Result<()> {
        unexpected_e()
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_tuple(self, _len: usize) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _i: u32,
        _v: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _i: u32,
        _v: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn collect_str<T: ?Sized + fmt::Display>(self, _value: &T) -> Result<()> {
        unexpected_e()
    }
    fn is_human_readable(&self) -> bool {
        true
    }
}
impl UsizeCapture {
    pub(super) fn finish(self) -> Result<usize> {
        self.v
            .ok_or_else(|| Error::unexpected("missing numeric ptr"))
    }
}

/// Minimal serializer that captures a boolean from a serialized field.
///
/// Used internally to read the `present` flag from weak-anchor payloads.
#[derive(Default)]
pub(super) struct BoolCapture {
    v: Option<bool>,
}
impl Serializer for &mut BoolCapture {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = ser::Impossible<(), Error>;
    type SerializeTuple = ser::Impossible<(), Error>;
    type SerializeTupleStruct = ser::Impossible<(), Error>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;
    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStruct = ser::Impossible<(), Error>;
    type SerializeStructVariant = ser::Impossible<(), Error>;

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.v = Some(v);
        Ok(())
    }
    fn serialize_i8(self, _v: i8) -> Result<()> {
        bool_expected_err()
    }
    fn serialize_i16(self, _v: i16) -> Result<()> {
        bool_expected_err()
    }
    fn serialize_i32(self, _v: i32) -> Result<()> {
        bool_expected_err()
    }
    fn serialize_i64(self, _v: i64) -> Result<()> {
        bool_expected_err()
    }
    fn serialize_u8(self, _v: u8) -> Result<()> {
        bool_expected_err()
    }
    fn serialize_u16(self, _v: u16) -> Result<()> {
        bool_expected_err()
    }
    fn serialize_u32(self, _v: u32) -> Result<()> {
        bool_expected_err()
    }
    fn serialize_u64(self, _v: u64) -> Result<()> {
        bool_expected_err()
    }
    fn serialize_f32(self, _v: f32) -> Result<()> {
        bool_expected_err()
    }
    fn serialize_f64(self, _v: f64) -> Result<()> {
        bool_expected_err()
    }
    fn serialize_char(self, _c: char) -> Result<()> {
        bool_expected_err()
    }
    fn serialize_str(self, _v: &str) -> Result<()> {
        bool_expected_err()
    }
    fn serialize_bytes(self, _v: &[u8]) -> Result<()> {
        bool_expected_err()
    }
    fn serialize_none(self) -> Result<()> {
        bool_expected_err()
    }
    fn serialize_some<T: ?Sized + Serialize>(self, _v: &T) -> Result<()> {
        bool_expected_err()
    }
    fn serialize_unit(self) -> Result<()> {
        bool_expected_err()
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        unexpected_e()
    }
    fn serialize_unit_variant(self, _name: &'static str, _i: u32, _v: &'static str) -> Result<()> {
        unexpected_e()
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<()> {
        unexpected_e()
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _i: u32,
        _v: &'static str,
        _value: &T,
    ) -> Result<()> {
        unexpected_e()
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_tuple(self, _len: usize) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _i: u32,
        _v: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _i: u32,
        _v: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn collect_str<T: ?Sized + fmt::Display>(self, _value: &T) -> Result<()> {
        unexpected_e()
    }
    fn is_human_readable(&self) -> bool {
        true
    }
}
impl BoolCapture {
    pub(super) fn finish(self) -> Result<bool> {
        self.v.ok_or_else(|| Error::unexpected("missing bool"))
    }
}

/// Minimal serializer that captures a string from a serialized field.
///
/// Used internally to read the comment text for the Commented wrapper.
#[derive(Default)]
pub(super) struct StrCapture {
    s: Option<String>,
}
impl Serializer for &mut StrCapture {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = ser::Impossible<(), Error>;
    type SerializeTuple = ser::Impossible<(), Error>;
    type SerializeTupleStruct = ser::Impossible<(), Error>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;
    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStruct = ser::Impossible<(), Error>;
    type SerializeStructVariant = ser::Impossible<(), Error>;

    fn serialize_str(self, v: &str) -> Result<()> {
        self.s = Some(v.to_string());
        Ok(())
    }

    fn serialize_bool(self, _v: bool) -> Result<()> {
        str_expected_err()
    }
    fn serialize_i8(self, _v: i8) -> Result<()> {
        str_expected_err()
    }
    fn serialize_i16(self, _v: i16) -> Result<()> {
        str_expected_err()
    }
    fn serialize_i32(self, _v: i32) -> Result<()> {
        str_expected_err()
    }
    fn serialize_i64(self, _v: i64) -> Result<()> {
        str_expected_err()
    }
    fn serialize_i128(self, _v: i128) -> Result<()> {
        str_expected_err()
    }
    fn serialize_u8(self, _v: u8) -> Result<()> {
        str_expected_err()
    }
    fn serialize_u16(self, _v: u16) -> Result<()> {
        str_expected_err()
    }
    fn serialize_u32(self, _v: u32) -> Result<()> {
        str_expected_err()
    }
    fn serialize_u64(self, _v: u64) -> Result<()> {
        str_expected_err()
    }
    fn serialize_u128(self, _v: u128) -> Result<()> {
        str_expected_err()
    }
    fn serialize_f32(self, _v: f32) -> Result<()> {
        str_expected_err()
    }
    fn serialize_f64(self, _v: f64) -> Result<()> {
        str_expected_err()
    }
    fn serialize_char(self, _c: char) -> Result<()> {
        str_expected_err()
    }
    fn serialize_bytes(self, _v: &[u8]) -> Result<()> {
        str_expected_err()
    }
    fn serialize_none(self) -> Result<()> {
        str_expected_err()
    }
    fn serialize_some<T: ?Sized + Serialize>(self, _value: &T) -> Result<()> {
        str_expected_err()
    }
    fn serialize_unit(self) -> Result<()> {
        str_expected_err()
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        str_expected_err()
    }
    fn serialize_unit_variant(self, _name: &'static str, _i: u32, _v: &'static str) -> Result<()> {
        str_expected_err()
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<()> {
        str_expected_err()
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _i: u32,
        _v: &'static str,
        _value: &T,
    ) -> Result<()> {
        str_expected_err()
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> {
        str_expected_err_impossible()
    }
    fn serialize_tuple(self, _len: usize) -> Result<ser::Impossible<(), Error>> {
        str_expected_err_impossible()
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        str_expected_err_impossible()
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _i: u32,
        _v: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        str_expected_err_impossible()
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> {
        str_expected_err_impossible()
    }
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        str_expected_err_impossible()
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _i: u32,
        _v: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        str_expected_err_impossible()
    }
    fn collect_str<T: ?Sized + fmt::Display>(self, _value: &T) -> Result<()> {
        str_expected_err()
    }
    fn is_human_readable(&self) -> bool {
        true
    }
}
impl StrCapture {
    pub(super) fn finish(self) -> Result<String> {
        self.s.ok_or_else(|| Error::unexpected("missing string"))
    }
}

// ------------------------------------------------------------
// Key scalar helper
// ------------------------------------------------------------

/// Serialize a key using a restricted scalar-only serializer into a `String`.
///
/// Called by map/struct serializers to ensure YAML keys are scalars.
pub(super) fn scalar_key_to_string<K: Serialize + ?Sized>(
    key: &K,
    yaml_12: bool,
) -> Result<String> {
    let mut s = String::new();
    {
        let mut ks = KeyScalarSink { s: &mut s, yaml_12 };
        key.serialize(&mut ks)?;
    }
    Ok(s)
}

struct KeyScalarSink<'a> {
    s: &'a mut String,
    yaml_12: bool,
}

impl<'a> Serializer for &'a mut KeyScalarSink<'a> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = ser::Impossible<(), Error>;
    type SerializeTuple = ser::Impossible<(), Error>;
    type SerializeTupleStruct = ser::Impossible<(), Error>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;
    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStruct = ser::Impossible<(), Error>;
    type SerializeStructVariant = ser::Impossible<(), Error>;

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.s.push_str(if v { "true" } else { "false" });
        Ok(())
    }
    fn serialize_i64(self, v: i64) -> Result<()> {
        let _ = write!(self.s, "{}", v);
        Ok(())
    }
    fn serialize_i32(self, v: i32) -> Result<()> {
        self.serialize_i64(v as i64)
    }
    fn serialize_i16(self, v: i16) -> Result<()> {
        self.serialize_i64(v as i64)
    }
    fn serialize_i8(self, v: i8) -> Result<()> {
        self.serialize_i64(v as i64)
    }
    fn serialize_i128(self, v: i128) -> Result<()> {
        let _ = write!(self.s, "{}", v);
        Ok(())
    }
    fn serialize_u64(self, v: u64) -> Result<()> {
        let _ = write!(self.s, "{}", v);
        Ok(())
    }
    fn serialize_u32(self, v: u32) -> Result<()> {
        self.serialize_u64(v as u64)
    }
    fn serialize_u16(self, v: u16) -> Result<()> {
        self.serialize_u64(v as u64)
    }
    fn serialize_u8(self, v: u8) -> Result<()> {
        self.serialize_u64(v as u64)
    }
    fn serialize_u128(self, v: u128) -> Result<()> {
        let _ = write!(self.s, "{}", v);
        Ok(())
    }
    fn serialize_f32(self, v: f32) -> Result<()> {
        zmij_format::push_float_string(self.s, v)
    }
    fn serialize_f64(self, v: f64) -> Result<()> {
        zmij_format::push_float_string(self.s, v)
    }

    fn serialize_char(self, v: char) -> Result<()> {
        let mut buf = [0u8; 4];
        self.serialize_str(v.encode_utf8(&mut buf))
    }
    fn serialize_str(self, v: &str) -> Result<()> {
        // Keys are in a more restrictive position than values (':' is structural),
        // but they also must avoid ambiguous plain scalars (e.g. YAML 1.1 bool spellings
        // like y/n/yes/no) to preserve intended string keys.
        // Be conservative here: keys may be emitted in both block and flow mappings,
        // and flow mappings treat characters like ','/[]/{} as structural.
        if is_plain_safe(v) && is_plain_value_safe(v, self.yaml_12, true) {
            self.s.push_str(v);
        } else {
            self.s.push('"');
            for ch in v.chars() {
                match ch {
                    '\\' => self.s.push_str("\\\\"),
                    '"' => self.s.push_str("\\\""),
                    '\n' => self.s.push_str("\\n"),
                    '\r' => self.s.push_str("\\r"),
                    '\t' => self.s.push_str("\\t"),
                    c if c.is_control() => {
                        use std::fmt::Write as _;
                        // Writing into a String cannot fail; ignore the Result to avoid unwrap.
                        let _ = write!(self.s, "\\u{:04X}", c as u32);
                    }
                    c => self.s.push(c),
                }
            }
            self.s.push('"');
        }
        Ok(())
    }
    fn serialize_bytes(self, _v: &[u8]) -> Result<()> {
        non_scalar_key_e()
    }
    fn serialize_none(self) -> Result<()> {
        self.s.push_str("null");
        Ok(())
    }
    fn serialize_some<T: ?Sized + Serialize>(self, v: &T) -> Result<()> {
        v.serialize(self)
    }
    fn serialize_unit(self) -> Result<()> {
        self.s.push_str("null");
        Ok(())
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.serialize_str(variant)
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<()> {
        // Treat newtype structs transparently. This allows common key wrappers like
        // `struct Key(String);` / `struct Id(u64);` to be emitted as scalar keys.
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<()> {
        non_scalar_key_e()
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> {
        non_scalar_key()
    }
    fn serialize_tuple(self, _len: usize) -> Result<ser::Impossible<(), Error>> {
        non_scalar_key()
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        non_scalar_key()
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        non_scalar_key()
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> {
        non_scalar_key()
    }
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        non_scalar_key()
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        non_scalar_key()
    }
    fn collect_str<T: ?Sized + fmt::Display>(self, v: &T) -> Result<()> {
        self.serialize_str(&v.to_string())
    }
    fn is_human_readable(&self) -> bool {
        true
    }
}

#[cold]
fn ptr_unsigned_err() -> Result<()> {
    Err(Error::unexpected("ptr expects unsigned integer"))
}

#[cold]
fn bool_expected_err() -> Result<()> {
    Err(Error::unexpected("bool expected"))
}

#[cold]
fn str_expected_err() -> Result<()> {
    Err(Error::unexpected("str expected"))
}

#[cold]
fn str_expected_err_impossible() -> Result<ser::Impossible<(), Error>> {
    Err(Error::unexpected("str expected"))
}

#[cold]
fn unexpected() -> Result<ser::Impossible<(), Error>> {
    Err(Error::unexpected("unexpected"))
}

#[cold]
fn unexpected_e() -> Result<()> {
    Err(Error::unexpected("unexpected"))
}

#[cold]
fn non_scalar_key() -> Result<ser::Impossible<(), Error>> {
    Err(Error::unexpected("non-scalar key"))
}

#[cold]
fn non_scalar_key_e() -> Result<()> {
    Err(Error::unexpected("non-scalar key"))
}

#[cfg(test)]
mod tests_internal {
    use super::*;

    #[test]
    fn test_push_float_string_coverage() {
        let mut s = String::new();

        // 1.0 -> no decimal, no exp -> line 48-50
        zmij_format::push_float_string(&mut s, 1.0f64).unwrap();
        assert!(s.contains(".0"));

        // 1e-10 -> exponent without plus sign -> line 35-36? wait, no, "1e-10" has minus sign.
        // We need exponent missing decimal, and exponent missing sign (+).
        s.clear();
        zmij_format::push_float_string(&mut s, 1e20f64).unwrap();
        // and f32 variations
        s.clear();
        zmij_format::push_float_string(&mut s, 1e30f32).unwrap();

        s.clear();
        zmij_format::push_float_string(&mut s, f32::NAN).unwrap();
        s.clear();
        zmij_format::push_float_string(&mut s, f32::INFINITY).unwrap();
        s.clear();
        zmij_format::push_float_string(&mut s, f32::NEG_INFINITY).unwrap();
    }

    #[test]
    fn test_write_float_string_coverage() {
        let mut s = String::new();
        zmij_format::write_float_string(&mut s, 1e20f64).unwrap();
        zmij_format::write_float_string(&mut s, 1e30f32).unwrap();
    }
}

#[test]
fn test_captures_coverage() {
    use serde::Serializer;
    let mut u = UsizeCapture::default();
    let _ = (&mut u).serialize_unit_struct("A");
    let _ = (&mut u).serialize_unit_variant("A", 0, "V");
    let _ = (&mut u).serialize_newtype_struct("A", &1);
    let _ = (&mut u).serialize_newtype_variant("A", 0, "V", &1);
    let _ = (&mut u).collect_str("A");
    let serializer = &mut u;
    assert!(Serializer::is_human_readable(&serializer));

    let mut b = BoolCapture::default();
    let _ = (&mut b).serialize_unit_struct("A");
    let _ = (&mut b).serialize_unit_variant("A", 0, "V");
    let _ = (&mut b).serialize_newtype_struct("A", &1);
    let _ = (&mut b).serialize_newtype_variant("A", 0, "V", &1);
    let _ = (&mut b).collect_str("A");
    let serializer = &mut b;
    assert!(Serializer::is_human_readable(&serializer));

    let mut st = StrCapture::default();
    let _ = (&mut st).serialize_unit_struct("A");
    let _ = (&mut st).serialize_unit_variant("A", 0, "V");
    let _ = (&mut st).serialize_newtype_struct("A", &1);
    let _ = (&mut st).serialize_newtype_variant("A", 0, "V", &1);
    let _ = (&mut st).collect_str("A");
    let serializer = &mut st;
    assert!(Serializer::is_human_readable(&serializer));
}
