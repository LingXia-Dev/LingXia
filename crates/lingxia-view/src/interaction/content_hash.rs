//! A hash of what a value serializes, fed straight from serde without building its text.
use serde::ser::{self, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher as _;

/// Equal for values whose serialized form is equal: same fields, same order, same values.
pub(crate) fn hash<T: Serialize + ?Sized>(value: &T) -> u64 {
    let mut state = DefaultHasher::new();
    // Every serializer call writes to the hasher; none can fail.
    let _ = value.serialize(Feed(&mut state));
    state.finish()
}

#[derive(Debug)]
pub(crate) struct Never;

impl std::fmt::Display for Never {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("content hash")
    }
}
impl std::error::Error for Never {}
impl ser::Error for Never {
    fn custom<T: std::fmt::Display>(_: T) -> Self {
        Never
    }
}

struct Feed<'a>(&'a mut DefaultHasher);

// Each kind writes a distinct tag, so an omitted optional field cannot alias its neighbour.
const NONE: u8 = 0;
const SOME: u8 = 1;
const UNIT: u8 = 2;
const SEQ: u8 = 3;
const MAP: u8 = 4;
const STRUCT: u8 = 5;
const VARIANT: u8 = 6;
const END: u8 = 7;
const STR: u8 = 8;

impl Feed<'_> {
    fn str(&mut self, value: &str) {
        self.0.write_u8(STR);
        self.0.write_usize(value.len());
        self.0.write(value.as_bytes());
    }
    fn variant(&mut self, index: u32) {
        self.0.write_u8(VARIANT);
        self.0.write_u32(index);
    }
}

impl<'a> ser::Serializer for Feed<'a> {
    type Ok = ();
    type Error = Never;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<(), Never> {
        self.0.write_u8(v as u8);
        Ok(())
    }
    fn serialize_i8(self, v: i8) -> Result<(), Never> {
        self.0.write_i64(v.into());
        Ok(())
    }
    fn serialize_i16(self, v: i16) -> Result<(), Never> {
        self.0.write_i64(v.into());
        Ok(())
    }
    fn serialize_i32(self, v: i32) -> Result<(), Never> {
        self.0.write_i64(v.into());
        Ok(())
    }
    fn serialize_i64(self, v: i64) -> Result<(), Never> {
        self.0.write_i64(v);
        Ok(())
    }
    fn serialize_u8(self, v: u8) -> Result<(), Never> {
        self.0.write_u64(v.into());
        Ok(())
    }
    fn serialize_u16(self, v: u16) -> Result<(), Never> {
        self.0.write_u64(v.into());
        Ok(())
    }
    fn serialize_u32(self, v: u32) -> Result<(), Never> {
        self.0.write_u64(v.into());
        Ok(())
    }
    fn serialize_u64(self, v: u64) -> Result<(), Never> {
        self.0.write_u64(v);
        Ok(())
    }
    fn serialize_f32(self, v: f32) -> Result<(), Never> {
        self.serialize_f64(v.into())
    }
    fn serialize_f64(self, v: f64) -> Result<(), Never> {
        // JSON writes -0.0 and 0.0 alike and every NaN as null.
        let v = if v == 0.0 { 0.0 } else { v };
        self.0
            .write_u64(if v.is_nan() { u64::MAX } else { v.to_bits() });
        Ok(())
    }
    fn serialize_char(mut self, v: char) -> Result<(), Never> {
        self.str(v.encode_utf8(&mut [0; 4]));
        Ok(())
    }
    fn serialize_str(mut self, v: &str) -> Result<(), Never> {
        self.str(v);
        Ok(())
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<(), Never> {
        self.0.write_u8(SEQ);
        self.0.write_usize(v.len());
        self.0.write(v);
        Ok(())
    }
    fn serialize_none(self) -> Result<(), Never> {
        self.0.write_u8(NONE);
        Ok(())
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<(), Never> {
        self.0.write_u8(SOME);
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), Never> {
        self.0.write_u8(UNIT);
        Ok(())
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Never> {
        self.serialize_unit()
    }
    fn serialize_unit_variant(
        mut self,
        _: &'static str,
        index: u32,
        _: &'static str,
    ) -> Result<(), Never> {
        self.variant(index);
        Ok(())
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<(), Never> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        mut self,
        _: &'static str,
        index: u32,
        _: &'static str,
        value: &T,
    ) -> Result<(), Never> {
        self.variant(index);
        value.serialize(self)
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self, Never> {
        self.0.write_u8(SEQ);
        Ok(self)
    }
    fn serialize_tuple(self, _: usize) -> Result<Self, Never> {
        self.serialize_seq(None)
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Self, Never> {
        self.serialize_seq(None)
    }
    fn serialize_tuple_variant(
        mut self,
        _: &'static str,
        index: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self, Never> {
        self.variant(index);
        self.serialize_seq(None)
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self, Never> {
        self.0.write_u8(MAP);
        Ok(self)
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self, Never> {
        self.0.write_u8(STRUCT);
        Ok(self)
    }
    fn serialize_struct_variant(
        mut self,
        _: &'static str,
        index: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self, Never> {
        self.variant(index);
        self.0.write_u8(STRUCT);
        Ok(self)
    }
}

impl ser::SerializeSeq for Feed<'_> {
    type Ok = ();
    type Error = Never;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Never> {
        value.serialize(Feed(&mut *self.0))
    }
    fn end(self) -> Result<(), Never> {
        self.0.write_u8(END);
        Ok(())
    }
}
impl ser::SerializeTuple for Feed<'_> {
    type Ok = ();
    type Error = Never;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Never> {
        value.serialize(Feed(&mut *self.0))
    }
    fn end(self) -> Result<(), Never> {
        ser::SerializeSeq::end(self)
    }
}
impl ser::SerializeTupleStruct for Feed<'_> {
    type Ok = ();
    type Error = Never;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Never> {
        value.serialize(Feed(&mut *self.0))
    }
    fn end(self) -> Result<(), Never> {
        ser::SerializeSeq::end(self)
    }
}
impl ser::SerializeTupleVariant for Feed<'_> {
    type Ok = ();
    type Error = Never;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Never> {
        value.serialize(Feed(&mut *self.0))
    }
    fn end(self) -> Result<(), Never> {
        ser::SerializeSeq::end(self)
    }
}
impl ser::SerializeMap for Feed<'_> {
    type Ok = ();
    type Error = Never;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Never> {
        key.serialize(Feed(&mut *self.0))
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Never> {
        value.serialize(Feed(&mut *self.0))
    }
    fn end(self) -> Result<(), Never> {
        ser::SerializeSeq::end(self)
    }
}
impl ser::SerializeStruct for Feed<'_> {
    type Ok = ();
    type Error = Never;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Never> {
        Feed(&mut *self.0).str(key);
        value.serialize(Feed(&mut *self.0))
    }
    fn end(self) -> Result<(), Never> {
        ser::SerializeSeq::end(self)
    }
}
impl ser::SerializeStructVariant for Feed<'_> {
    type Ok = ();
    type Error = Never;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Never> {
        ser::SerializeStruct::serialize_field(self, key, value)
    }
    fn end(self) -> Result<(), Never> {
        ser::SerializeSeq::end(self)
    }
}

#[cfg(test)]
mod tests {
    use super::hash;

    #[derive(serde::Serialize)]
    struct Node {
        name: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        a: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        b: Option<u32>,
        bounds: [f32; 2],
        #[serde(skip)]
        _local: u8,
    }
    const fn node(a: Option<u32>, b: Option<u32>, x: f32, local: u8) -> Node {
        Node {
            name: "n",
            a,
            b,
            bounds: [x, 1.0],
            _local: local,
        }
    }

    #[test]
    fn equal_serialized_forms_hash_equal_and_others_differ() {
        assert_eq!(
            hash(&node(Some(1), None, 0.0, 0)),
            hash(&node(Some(1), None, -0.0, 9))
        );
        assert_ne!(
            hash(&node(Some(1), None, 0.0, 0)),
            hash(&node(None, Some(1), 0.0, 0))
        );
        assert_ne!(
            hash(&node(None, None, 0.5, 0)),
            hash(&node(None, None, 0.25, 0))
        );
        assert_ne!(hash(&[vec![1u8], vec![]]), hash(&[vec![], vec![1u8]]));
    }
}
