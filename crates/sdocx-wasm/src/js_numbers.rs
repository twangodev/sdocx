use serde::ser::{self, Serialize, Serializer};
use std::fmt::Display;

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// Borrowed serialization with exact decimal transport for unsafe JS integers.
pub(crate) struct JsSafe<T>(pub(crate) T);

impl<T: Serialize> Serialize for JsSafe<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(Numbers(serializer))
    }
}

struct Numbers<S>(S);

macro_rules! delegate_scalars {
    ($($method:ident($ty:ty)),* $(,)?) => {
        $(fn $method(self, value: $ty) -> Result<Self::Ok, Self::Error> {
            self.0.$method(value)
        })*
    };
}

macro_rules! delegate_containers {
    ($($method:ident -> $output:ident ($($arg:ident: $ty:ty),*)),* $(,)?) => {
        $(fn $method(self, $($arg: $ty),*) -> Result<Self::$output, Self::Error> {
            self.0.$method($($arg),*).map(Numbers)
        })*
    };
}

impl<S: Serializer> Serializer for Numbers<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = Numbers<S::SerializeSeq>;
    type SerializeTuple = Numbers<S::SerializeTuple>;
    type SerializeTupleStruct = Numbers<S::SerializeTupleStruct>;
    type SerializeTupleVariant = Numbers<S::SerializeTupleVariant>;
    type SerializeMap = Numbers<S::SerializeMap>;
    type SerializeStruct = Numbers<S::SerializeStruct>;
    type SerializeStructVariant = Numbers<S::SerializeStructVariant>;

    delegate_scalars! {
        serialize_bool(bool), serialize_i8(i8), serialize_i16(i16), serialize_i32(i32),
        serialize_u8(u8), serialize_u16(u16), serialize_u32(u32),
        serialize_i128(i128), serialize_u128(u128), serialize_f32(f32), serialize_f64(f64),
        serialize_char(char), serialize_str(&str), serialize_bytes(&[u8]),
    }

    fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
        if value.unsigned_abs() > MAX_SAFE_INTEGER {
            self.0.serialize_str(&value.to_string())
        } else {
            self.0.serialize_i64(value)
        }
    }

    fn serialize_u64(self, value: u64) -> Result<Self::Ok, Self::Error> {
        if value > MAX_SAFE_INTEGER {
            self.0.serialize_str(&value.to_string())
        } else {
            self.0.serialize_u64(value)
        }
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_none()
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_some(&JsSafe(value))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_unit()
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_unit_struct(name)
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_unit_variant(name, index, variant)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_newtype_struct(name, &JsSafe(value))
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.0
            .serialize_newtype_variant(name, index, variant, &JsSafe(value))
    }

    delegate_containers! {
        serialize_seq -> SerializeSeq (len: Option<usize>),
        serialize_tuple -> SerializeTuple (len: usize),
        serialize_tuple_struct -> SerializeTupleStruct (name: &'static str, len: usize),
        serialize_tuple_variant -> SerializeTupleVariant (name: &'static str, index: u32, variant: &'static str, len: usize),
        serialize_map -> SerializeMap (len: Option<usize>),
        serialize_struct -> SerializeStruct (name: &'static str, len: usize),
        serialize_struct_variant -> SerializeStructVariant (name: &'static str, index: u32, variant: &'static str, len: usize),
    }

    fn collect_str<T: ?Sized + Display>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        self.0.collect_str(value)
    }

    fn is_human_readable(&self) -> bool {
        self.0.is_human_readable()
    }
}

macro_rules! sequence {
    ($($trait:ident::$method:ident),* $(,)?) => {
        $(impl<S: ser::$trait> ser::$trait for Numbers<S> {
            type Ok = S::Ok;
            type Error = S::Error;
            fn $method<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
                self.0.$method(&JsSafe(value))
            }
            fn end(self) -> Result<Self::Ok, Self::Error> {
                self.0.end()
            }
        })*
    };
}

sequence! {
    SerializeSeq::serialize_element,
    SerializeTuple::serialize_element,
    SerializeTupleStruct::serialize_field,
    SerializeTupleVariant::serialize_field,
}

impl<S: ser::SerializeMap> ser::SerializeMap for Numbers<S> {
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Self::Error> {
        self.0.serialize_key(&JsSafe(key))
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.0.serialize_value(&JsSafe(value))
    }

    fn serialize_entry<K: ?Sized + Serialize, V: ?Sized + Serialize>(
        &mut self,
        key: &K,
        value: &V,
    ) -> Result<(), Self::Error> {
        self.0.serialize_entry(&JsSafe(key), &JsSafe(value))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.0.end()
    }
}

macro_rules! structure {
    ($($trait:ident),* $(,)?) => {
        $(impl<S: ser::$trait> ser::$trait for Numbers<S> {
            type Ok = S::Ok;
            type Error = S::Error;
            fn serialize_field<T: ?Sized + Serialize>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error> {
                self.0.serialize_field(key, &JsSafe(value))
            }
            fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
                self.0.skip_field(key)
            }
            fn end(self) -> Result<Self::Ok, Self::Error> {
                self.0.end()
            }
        })*
    };
}

structure! { SerializeStruct, SerializeStructVariant }

#[cfg(test)]
#[path = "js_numbers_tests.rs"]
mod tests;
