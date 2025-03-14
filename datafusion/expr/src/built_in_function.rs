// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

//! Built-in functions module contains all the built-in functions definitions.

use crate::nullif::SUPPORTED_NULLIF_TYPES;
use crate::type_coercion::functions::data_types;
use crate::{
    conditional_expressions, struct_expressions, utils, Signature, TypeSignature,
    Volatility,
};
use arrow::datatypes::{DataType, Field, Fields, IntervalUnit, TimeUnit};
use datafusion_common::{internal_err, plan_err, DataFusionError, Result};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, OnceLock};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

/// Enum of all built-in scalar functions
// Contributor's guide for adding new scalar functions
// https://arrow.apache.org/datafusion/contributor-guide/index.html#how-to-add-a-new-scalar-function
#[derive(Debug, Clone, PartialEq, Eq, Hash, EnumIter, Copy)]
pub enum BuiltinScalarFunction {
    // math functions
    /// abs
    Abs,
    /// acos
    Acos,
    /// asin
    Asin,
    /// atan
    Atan,
    /// atan2
    Atan2,
    /// acosh
    Acosh,
    /// asinh
    Asinh,
    /// atanh
    Atanh,
    /// cbrt
    Cbrt,
    /// ceil
    Ceil,
    /// coalesce
    Coalesce,
    /// cos
    Cos,
    /// cos
    Cosh,
    /// Decode
    Decode,
    /// degrees
    Degrees,
    /// Digest
    Digest,
    /// Encode
    Encode,
    /// exp
    Exp,
    /// factorial
    Factorial,
    /// floor
    Floor,
    /// gcd, Greatest common divisor
    Gcd,
    /// lcm, Least common multiple
    Lcm,
    /// isnan
    Isnan,
    /// iszero
    Iszero,
    /// ln, Natural logarithm
    Ln,
    /// log, same as log10
    Log,
    /// log10
    Log10,
    /// log2
    Log2,
    /// nanvl
    Nanvl,
    /// pi
    Pi,
    /// power
    Power,
    /// radians
    Radians,
    /// round
    Round,
    /// signum
    Signum,
    /// sin
    Sin,
    /// sinh
    Sinh,
    /// sqrt
    Sqrt,
    /// tan
    Tan,
    /// tanh
    Tanh,
    /// trunc
    Trunc,
    /// cot
    Cot,

    // array functions
    /// array_append
    ArrayAppend,
    /// array_concat
    ArrayConcat,
    /// array_has
    ArrayHas,
    /// array_has_all
    ArrayHasAll,
    /// array_has_any
    ArrayHasAny,
    /// array_pop_back
    ArrayPopBack,
    /// array_dims
    ArrayDims,
    /// array_element
    ArrayElement,
    /// array_empty
    ArrayEmpty,
    /// array_length
    ArrayLength,
    /// array_ndims
    ArrayNdims,
    /// array_position
    ArrayPosition,
    /// array_positions
    ArrayPositions,
    /// array_prepend
    ArrayPrepend,
    /// array_remove
    ArrayRemove,
    /// array_remove_n
    ArrayRemoveN,
    /// array_remove_all
    ArrayRemoveAll,
    /// array_repeat
    ArrayRepeat,
    /// array_replace
    ArrayReplace,
    /// array_replace_n
    ArrayReplaceN,
    /// array_replace_all
    ArrayReplaceAll,
    /// array_slice
    ArraySlice,
    /// array_to_string
    ArrayToString,
    /// cardinality
    Cardinality,
    /// construct an array from columns
    MakeArray,
    /// Flatten
    Flatten,

    // struct functions
    /// struct
    Struct,

    // string functions
    /// ascii
    Ascii,
    /// bit_length
    BitLength,
    /// btrim
    Btrim,
    /// character_length
    CharacterLength,
    /// chr
    Chr,
    /// concat
    Concat,
    /// concat_ws
    ConcatWithSeparator,
    /// date_part
    DatePart,
    /// date_trunc
    DateTrunc,
    /// date_bin
    DateBin,
    /// initcap
    InitCap,
    /// left
    Left,
    /// lpad
    Lpad,
    /// lower
    Lower,
    /// ltrim
    Ltrim,
    /// md5
    MD5,
    /// nullif
    NullIf,
    /// octet_length
    OctetLength,
    /// random
    Random,
    /// regexp_replace
    RegexpReplace,
    /// repeat
    Repeat,
    /// replace
    Replace,
    /// reverse
    Reverse,
    /// right
    Right,
    /// rpad
    Rpad,
    /// rtrim
    Rtrim,
    /// sha224
    SHA224,
    /// sha256
    SHA256,
    /// sha384
    SHA384,
    /// Sha512
    SHA512,
    /// split_part
    SplitPart,
    /// string_to_array
    StringToArray,
    /// starts_with
    StartsWith,
    /// strpos
    Strpos,
    /// substr
    Substr,
    /// to_hex
    ToHex,
    /// to_timestamp
    ToTimestamp,
    /// to_timestamp_millis
    ToTimestampMillis,
    /// to_timestamp_micros
    ToTimestampMicros,
    /// to_timestamp_seconds
    ToTimestampSeconds,
    /// from_unixtime
    FromUnixtime,
    ///now
    Now,
    ///current_date
    CurrentDate,
    /// current_time
    CurrentTime,
    /// translate
    Translate,
    /// trim
    Trim,
    /// upper
    Upper,
    /// uuid
    Uuid,
    /// regexp_match
    RegexpMatch,
    /// arrow_typeof
    ArrowTypeof,
}

/// Maps the sql function name to `BuiltinScalarFunction`
fn name_to_function() -> &'static HashMap<&'static str, BuiltinScalarFunction> {
    static NAME_TO_FUNCTION_LOCK: OnceLock<HashMap<&'static str, BuiltinScalarFunction>> =
        OnceLock::new();
    NAME_TO_FUNCTION_LOCK.get_or_init(|| {
        let mut map = HashMap::new();
        BuiltinScalarFunction::iter().for_each(|func| {
            let a = aliases(&func);
            a.iter().for_each(|&a| {
                map.insert(a, func);
            });
        });
        map
    })
}

/// Maps `BuiltinScalarFunction` --> canonical sql function
/// First alias in the array is used to display function names
fn function_to_name() -> &'static HashMap<BuiltinScalarFunction, &'static str> {
    static FUNCTION_TO_NAME_LOCK: OnceLock<HashMap<BuiltinScalarFunction, &'static str>> =
        OnceLock::new();
    FUNCTION_TO_NAME_LOCK.get_or_init(|| {
        let mut map = HashMap::new();
        BuiltinScalarFunction::iter().for_each(|func| {
            map.insert(func, *aliases(&func).first().unwrap_or(&"NO_ALIAS"));
        });
        map
    })
}

impl BuiltinScalarFunction {
    /// an allowlist of functions to take zero arguments, so that they will get special treatment
    /// while executing.
    pub fn supports_zero_argument(&self) -> bool {
        matches!(
            self,
            BuiltinScalarFunction::Pi
                | BuiltinScalarFunction::Random
                | BuiltinScalarFunction::Now
                | BuiltinScalarFunction::CurrentDate
                | BuiltinScalarFunction::CurrentTime
                | BuiltinScalarFunction::Uuid
                | BuiltinScalarFunction::MakeArray
        )
    }
    /// Returns the [Volatility] of the builtin function.
    pub fn volatility(&self) -> Volatility {
        match self {
            // Immutable scalar builtins
            BuiltinScalarFunction::Abs => Volatility::Immutable,
            BuiltinScalarFunction::Acos => Volatility::Immutable,
            BuiltinScalarFunction::Asin => Volatility::Immutable,
            BuiltinScalarFunction::Atan => Volatility::Immutable,
            BuiltinScalarFunction::Atan2 => Volatility::Immutable,
            BuiltinScalarFunction::Acosh => Volatility::Immutable,
            BuiltinScalarFunction::Asinh => Volatility::Immutable,
            BuiltinScalarFunction::Atanh => Volatility::Immutable,
            BuiltinScalarFunction::Ceil => Volatility::Immutable,
            BuiltinScalarFunction::Coalesce => Volatility::Immutable,
            BuiltinScalarFunction::Cos => Volatility::Immutable,
            BuiltinScalarFunction::Cosh => Volatility::Immutable,
            BuiltinScalarFunction::Decode => Volatility::Immutable,
            BuiltinScalarFunction::Degrees => Volatility::Immutable,
            BuiltinScalarFunction::Encode => Volatility::Immutable,
            BuiltinScalarFunction::Exp => Volatility::Immutable,
            BuiltinScalarFunction::Factorial => Volatility::Immutable,
            BuiltinScalarFunction::Floor => Volatility::Immutable,
            BuiltinScalarFunction::Gcd => Volatility::Immutable,
            BuiltinScalarFunction::Isnan => Volatility::Immutable,
            BuiltinScalarFunction::Iszero => Volatility::Immutable,
            BuiltinScalarFunction::Lcm => Volatility::Immutable,
            BuiltinScalarFunction::Ln => Volatility::Immutable,
            BuiltinScalarFunction::Log => Volatility::Immutable,
            BuiltinScalarFunction::Log10 => Volatility::Immutable,
            BuiltinScalarFunction::Log2 => Volatility::Immutable,
            BuiltinScalarFunction::Nanvl => Volatility::Immutable,
            BuiltinScalarFunction::Pi => Volatility::Immutable,
            BuiltinScalarFunction::Power => Volatility::Immutable,
            BuiltinScalarFunction::Round => Volatility::Immutable,
            BuiltinScalarFunction::Signum => Volatility::Immutable,
            BuiltinScalarFunction::Sin => Volatility::Immutable,
            BuiltinScalarFunction::Sinh => Volatility::Immutable,
            BuiltinScalarFunction::Sqrt => Volatility::Immutable,
            BuiltinScalarFunction::Cbrt => Volatility::Immutable,
            BuiltinScalarFunction::Cot => Volatility::Immutable,
            BuiltinScalarFunction::Tan => Volatility::Immutable,
            BuiltinScalarFunction::Tanh => Volatility::Immutable,
            BuiltinScalarFunction::Trunc => Volatility::Immutable,
            BuiltinScalarFunction::ArrayAppend => Volatility::Immutable,
            BuiltinScalarFunction::ArrayConcat => Volatility::Immutable,
            BuiltinScalarFunction::ArrayEmpty => Volatility::Immutable,
            BuiltinScalarFunction::ArrayHasAll => Volatility::Immutable,
            BuiltinScalarFunction::ArrayHasAny => Volatility::Immutable,
            BuiltinScalarFunction::ArrayHas => Volatility::Immutable,
            BuiltinScalarFunction::ArrayDims => Volatility::Immutable,
            BuiltinScalarFunction::ArrayElement => Volatility::Immutable,
            BuiltinScalarFunction::ArrayLength => Volatility::Immutable,
            BuiltinScalarFunction::ArrayNdims => Volatility::Immutable,
            BuiltinScalarFunction::ArrayPopBack => Volatility::Immutable,
            BuiltinScalarFunction::ArrayPosition => Volatility::Immutable,
            BuiltinScalarFunction::ArrayPositions => Volatility::Immutable,
            BuiltinScalarFunction::ArrayPrepend => Volatility::Immutable,
            BuiltinScalarFunction::ArrayRepeat => Volatility::Immutable,
            BuiltinScalarFunction::ArrayRemove => Volatility::Immutable,
            BuiltinScalarFunction::ArrayRemoveN => Volatility::Immutable,
            BuiltinScalarFunction::ArrayRemoveAll => Volatility::Immutable,
            BuiltinScalarFunction::ArrayReplace => Volatility::Immutable,
            BuiltinScalarFunction::ArrayReplaceN => Volatility::Immutable,
            BuiltinScalarFunction::ArrayReplaceAll => Volatility::Immutable,
            BuiltinScalarFunction::Flatten => Volatility::Immutable,
            BuiltinScalarFunction::ArraySlice => Volatility::Immutable,
            BuiltinScalarFunction::ArrayToString => Volatility::Immutable,
            BuiltinScalarFunction::Cardinality => Volatility::Immutable,
            BuiltinScalarFunction::MakeArray => Volatility::Immutable,
            BuiltinScalarFunction::Ascii => Volatility::Immutable,
            BuiltinScalarFunction::BitLength => Volatility::Immutable,
            BuiltinScalarFunction::Btrim => Volatility::Immutable,
            BuiltinScalarFunction::CharacterLength => Volatility::Immutable,
            BuiltinScalarFunction::Chr => Volatility::Immutable,
            BuiltinScalarFunction::Concat => Volatility::Immutable,
            BuiltinScalarFunction::ConcatWithSeparator => Volatility::Immutable,
            BuiltinScalarFunction::DatePart => Volatility::Immutable,
            BuiltinScalarFunction::DateTrunc => Volatility::Immutable,
            BuiltinScalarFunction::DateBin => Volatility::Immutable,
            BuiltinScalarFunction::InitCap => Volatility::Immutable,
            BuiltinScalarFunction::Left => Volatility::Immutable,
            BuiltinScalarFunction::Lpad => Volatility::Immutable,
            BuiltinScalarFunction::Lower => Volatility::Immutable,
            BuiltinScalarFunction::Ltrim => Volatility::Immutable,
            BuiltinScalarFunction::MD5 => Volatility::Immutable,
            BuiltinScalarFunction::NullIf => Volatility::Immutable,
            BuiltinScalarFunction::OctetLength => Volatility::Immutable,
            BuiltinScalarFunction::Radians => Volatility::Immutable,
            BuiltinScalarFunction::RegexpReplace => Volatility::Immutable,
            BuiltinScalarFunction::Repeat => Volatility::Immutable,
            BuiltinScalarFunction::Replace => Volatility::Immutable,
            BuiltinScalarFunction::Reverse => Volatility::Immutable,
            BuiltinScalarFunction::Right => Volatility::Immutable,
            BuiltinScalarFunction::Rpad => Volatility::Immutable,
            BuiltinScalarFunction::Rtrim => Volatility::Immutable,
            BuiltinScalarFunction::SHA224 => Volatility::Immutable,
            BuiltinScalarFunction::SHA256 => Volatility::Immutable,
            BuiltinScalarFunction::SHA384 => Volatility::Immutable,
            BuiltinScalarFunction::SHA512 => Volatility::Immutable,
            BuiltinScalarFunction::Digest => Volatility::Immutable,
            BuiltinScalarFunction::SplitPart => Volatility::Immutable,
            BuiltinScalarFunction::StringToArray => Volatility::Immutable,
            BuiltinScalarFunction::StartsWith => Volatility::Immutable,
            BuiltinScalarFunction::Strpos => Volatility::Immutable,
            BuiltinScalarFunction::Substr => Volatility::Immutable,
            BuiltinScalarFunction::ToHex => Volatility::Immutable,
            BuiltinScalarFunction::ToTimestamp => Volatility::Immutable,
            BuiltinScalarFunction::ToTimestampMillis => Volatility::Immutable,
            BuiltinScalarFunction::ToTimestampMicros => Volatility::Immutable,
            BuiltinScalarFunction::ToTimestampSeconds => Volatility::Immutable,
            BuiltinScalarFunction::Translate => Volatility::Immutable,
            BuiltinScalarFunction::Trim => Volatility::Immutable,
            BuiltinScalarFunction::Upper => Volatility::Immutable,
            BuiltinScalarFunction::RegexpMatch => Volatility::Immutable,
            BuiltinScalarFunction::Struct => Volatility::Immutable,
            BuiltinScalarFunction::FromUnixtime => Volatility::Immutable,
            BuiltinScalarFunction::ArrowTypeof => Volatility::Immutable,

            // Stable builtin functions
            BuiltinScalarFunction::Now => Volatility::Stable,
            BuiltinScalarFunction::CurrentDate => Volatility::Stable,
            BuiltinScalarFunction::CurrentTime => Volatility::Stable,

            // Volatile builtin functions
            BuiltinScalarFunction::Random => Volatility::Volatile,
            BuiltinScalarFunction::Uuid => Volatility::Volatile,
        }
    }

    /// Returns the dimension [`DataType`] of [`DataType::List`] if
    /// treated as a N-dimensional array.
    ///
    /// ## Examples:
    ///
    /// * `Int64` has dimension 1
    /// * `List(Int64)` has dimension 2
    /// * `List(List(Int64))` has dimension 3
    /// * etc.
    fn return_dimension(self, input_expr_type: DataType) -> u64 {
        let mut res: u64 = 1;
        let mut current_data_type = input_expr_type;
        loop {
            match current_data_type {
                DataType::List(field) => {
                    current_data_type = field.data_type().clone();
                    res += 1;
                }
                _ => return res,
            }
        }
    }

    /// Returns the output [`DataType`] of this function
    pub fn return_type(self, input_expr_types: &[DataType]) -> Result<DataType> {
        use DataType::*;
        use TimeUnit::*;

        // Note that this function *must* return the same type that the respective physical expression returns
        // or the execution panics.

        if input_expr_types.is_empty() && !self.supports_zero_argument() {
            return plan_err!(
                "{}",
                utils::generate_signature_error_msg(
                    &format!("{self}"),
                    self.signature(),
                    input_expr_types
                )
            );
        }

        // verify that this is a valid set of data types for this function
        data_types(input_expr_types, &self.signature()).map_err(|_| {
            DataFusionError::Plan(utils::generate_signature_error_msg(
                &format!("{self}"),
                self.signature(),
                input_expr_types,
            ))
        })?;

        // the return type of the built in function.
        // Some built-in functions' return type depends on the incoming type.
        match self {
            BuiltinScalarFunction::Flatten => {
                fn get_base_type(data_type: &DataType) -> Result<DataType> {
                    match data_type {
                        DataType::List(field) => match field.data_type() {
                            DataType::List(_) => get_base_type(field.data_type()),
                            _ => Ok(data_type.to_owned()),
                        },
                        _ => internal_err!("Not reachable, data_type should be List"),
                    }
                }

                let data_type = get_base_type(&input_expr_types[0])?;
                Ok(data_type)
            }
            BuiltinScalarFunction::ArrayAppend => Ok(input_expr_types[0].clone()),
            BuiltinScalarFunction::ArrayConcat => {
                let mut expr_type = Null;
                let mut max_dims = 0;
                for input_expr_type in input_expr_types {
                    match input_expr_type {
                        List(field) => {
                            if !field.data_type().equals_datatype(&Null) {
                                let dims = self.return_dimension(input_expr_type.clone());
                                if max_dims < dims {
                                    max_dims = dims;
                                    expr_type = input_expr_type.clone();
                                }
                            }
                        }
                        _ => {
                            return plan_err!(
                                "The {self} function can only accept list as the args."
                            )
                        }
                    }
                }

                Ok(expr_type)
            }
            BuiltinScalarFunction::ArrayHasAll
            | BuiltinScalarFunction::ArrayHasAny
            | BuiltinScalarFunction::ArrayHas
            | BuiltinScalarFunction::ArrayEmpty => Ok(Boolean),
            BuiltinScalarFunction::ArrayDims => {
                Ok(List(Arc::new(Field::new("item", UInt64, true))))
            }
            BuiltinScalarFunction::ArrayElement => match &input_expr_types[0] {
                List(field) => Ok(field.data_type().clone()),
                _ => plan_err!(
                    "The {self} function can only accept list as the first argument"
                ),
            },
            BuiltinScalarFunction::ArrayLength => Ok(UInt64),
            BuiltinScalarFunction::ArrayNdims => Ok(UInt64),
            BuiltinScalarFunction::ArrayPopBack => Ok(input_expr_types[0].clone()),
            BuiltinScalarFunction::ArrayPosition => Ok(UInt64),
            BuiltinScalarFunction::ArrayPositions => {
                Ok(List(Arc::new(Field::new("item", UInt64, true))))
            }
            BuiltinScalarFunction::ArrayPrepend => Ok(input_expr_types[1].clone()),
            BuiltinScalarFunction::ArrayRepeat => Ok(List(Arc::new(Field::new(
                "item",
                input_expr_types[0].clone(),
                true,
            )))),
            BuiltinScalarFunction::ArrayRemove => Ok(input_expr_types[0].clone()),
            BuiltinScalarFunction::ArrayRemoveN => Ok(input_expr_types[0].clone()),
            BuiltinScalarFunction::ArrayRemoveAll => Ok(input_expr_types[0].clone()),
            BuiltinScalarFunction::ArrayReplace => Ok(input_expr_types[0].clone()),
            BuiltinScalarFunction::ArrayReplaceN => Ok(input_expr_types[0].clone()),
            BuiltinScalarFunction::ArrayReplaceAll => Ok(input_expr_types[0].clone()),
            BuiltinScalarFunction::ArraySlice => Ok(input_expr_types[0].clone()),
            BuiltinScalarFunction::ArrayToString => Ok(Utf8),
            BuiltinScalarFunction::Cardinality => Ok(UInt64),
            BuiltinScalarFunction::MakeArray => match input_expr_types.len() {
                0 => Ok(List(Arc::new(Field::new("item", Null, true)))),
                _ => {
                    let mut expr_type = Null;
                    for input_expr_type in input_expr_types {
                        if !input_expr_type.equals_datatype(&Null) {
                            expr_type = input_expr_type.clone();
                            break;
                        }
                    }

                    Ok(List(Arc::new(Field::new("item", expr_type, true))))
                }
            },
            BuiltinScalarFunction::Ascii => Ok(Int32),
            BuiltinScalarFunction::BitLength => {
                utf8_to_int_type(&input_expr_types[0], "bit_length")
            }
            BuiltinScalarFunction::Btrim => {
                utf8_to_str_type(&input_expr_types[0], "btrim")
            }
            BuiltinScalarFunction::CharacterLength => {
                utf8_to_int_type(&input_expr_types[0], "character_length")
            }
            BuiltinScalarFunction::Chr => Ok(Utf8),
            BuiltinScalarFunction::Coalesce => {
                // COALESCE has multiple args and they might get coerced, get a preview of this
                let coerced_types = data_types(input_expr_types, &self.signature());
                coerced_types.map(|types| types[0].clone())
            }
            BuiltinScalarFunction::Concat => Ok(Utf8),
            BuiltinScalarFunction::ConcatWithSeparator => Ok(Utf8),
            BuiltinScalarFunction::DatePart => Ok(Float64),
            BuiltinScalarFunction::DateBin | BuiltinScalarFunction::DateTrunc => {
                match input_expr_types[1] {
                    Timestamp(Nanosecond, _) | Utf8 | Null => {
                        Ok(Timestamp(Nanosecond, None))
                    }
                    Timestamp(Microsecond, _) => Ok(Timestamp(Microsecond, None)),
                    Timestamp(Millisecond, _) => Ok(Timestamp(Millisecond, None)),
                    Timestamp(Second, _) => Ok(Timestamp(Second, None)),
                    _ => plan_err!(
                    "The {self} function can only accept timestamp as the second arg."
                ),
                }
            }
            BuiltinScalarFunction::InitCap => {
                utf8_to_str_type(&input_expr_types[0], "initcap")
            }
            BuiltinScalarFunction::Left => utf8_to_str_type(&input_expr_types[0], "left"),
            BuiltinScalarFunction::Lower => {
                utf8_to_str_type(&input_expr_types[0], "lower")
            }
            BuiltinScalarFunction::Lpad => utf8_to_str_type(&input_expr_types[0], "lpad"),
            BuiltinScalarFunction::Ltrim => {
                utf8_to_str_type(&input_expr_types[0], "ltrim")
            }
            BuiltinScalarFunction::MD5 => utf8_to_str_type(&input_expr_types[0], "md5"),
            BuiltinScalarFunction::NullIf => {
                // NULLIF has two args and they might get coerced, get a preview of this
                let coerced_types = data_types(input_expr_types, &self.signature());
                coerced_types.map(|typs| typs[0].clone())
            }
            BuiltinScalarFunction::OctetLength => {
                utf8_to_int_type(&input_expr_types[0], "octet_length")
            }
            BuiltinScalarFunction::Pi => Ok(Float64),
            BuiltinScalarFunction::Random => Ok(Float64),
            BuiltinScalarFunction::Uuid => Ok(Utf8),
            BuiltinScalarFunction::RegexpReplace => {
                utf8_to_str_type(&input_expr_types[0], "regexp_replace")
            }
            BuiltinScalarFunction::Repeat => {
                utf8_to_str_type(&input_expr_types[0], "repeat")
            }
            BuiltinScalarFunction::Replace => {
                utf8_to_str_type(&input_expr_types[0], "replace")
            }
            BuiltinScalarFunction::Reverse => {
                utf8_to_str_type(&input_expr_types[0], "reverse")
            }
            BuiltinScalarFunction::Right => {
                utf8_to_str_type(&input_expr_types[0], "right")
            }
            BuiltinScalarFunction::Rpad => utf8_to_str_type(&input_expr_types[0], "rpad"),
            BuiltinScalarFunction::Rtrim => {
                utf8_to_str_type(&input_expr_types[0], "rtrim")
            }
            BuiltinScalarFunction::SHA224 => {
                utf8_or_binary_to_binary_type(&input_expr_types[0], "sha224")
            }
            BuiltinScalarFunction::SHA256 => {
                utf8_or_binary_to_binary_type(&input_expr_types[0], "sha256")
            }
            BuiltinScalarFunction::SHA384 => {
                utf8_or_binary_to_binary_type(&input_expr_types[0], "sha384")
            }
            BuiltinScalarFunction::SHA512 => {
                utf8_or_binary_to_binary_type(&input_expr_types[0], "sha512")
            }
            BuiltinScalarFunction::Digest => {
                utf8_or_binary_to_binary_type(&input_expr_types[0], "digest")
            }
            BuiltinScalarFunction::Encode => Ok(match input_expr_types[0] {
                Utf8 => Utf8,
                LargeUtf8 => LargeUtf8,
                Binary => Utf8,
                LargeBinary => LargeUtf8,
                Null => Null,
                _ => {
                    return plan_err!(
                        "The encode function can only accept utf8 or binary."
                    );
                }
            }),
            BuiltinScalarFunction::Decode => Ok(match input_expr_types[0] {
                Utf8 => Binary,
                LargeUtf8 => LargeBinary,
                Binary => Binary,
                LargeBinary => LargeBinary,
                Null => Null,
                _ => {
                    return plan_err!(
                        "The decode function can only accept utf8 or binary."
                    );
                }
            }),
            BuiltinScalarFunction::SplitPart => {
                utf8_to_str_type(&input_expr_types[0], "split_part")
            }
            BuiltinScalarFunction::StringToArray => Ok(List(Arc::new(Field::new(
                "item",
                input_expr_types[0].clone(),
                true,
            )))),
            BuiltinScalarFunction::StartsWith => Ok(Boolean),
            BuiltinScalarFunction::Strpos => {
                utf8_to_int_type(&input_expr_types[0], "strpos")
            }
            BuiltinScalarFunction::Substr => {
                utf8_to_str_type(&input_expr_types[0], "substr")
            }
            BuiltinScalarFunction::ToHex => Ok(match input_expr_types[0] {
                Int8 | Int16 | Int32 | Int64 => Utf8,
                _ => {
                    return plan_err!("The to_hex function can only accept integers.");
                }
            }),
            BuiltinScalarFunction::ToTimestamp => Ok(Timestamp(Nanosecond, None)),
            BuiltinScalarFunction::ToTimestampMillis => Ok(Timestamp(Millisecond, None)),
            BuiltinScalarFunction::ToTimestampMicros => Ok(Timestamp(Microsecond, None)),
            BuiltinScalarFunction::ToTimestampSeconds => Ok(Timestamp(Second, None)),
            BuiltinScalarFunction::FromUnixtime => Ok(Timestamp(Second, None)),
            BuiltinScalarFunction::Now => {
                Ok(Timestamp(Nanosecond, Some("+00:00".into())))
            }
            BuiltinScalarFunction::CurrentDate => Ok(Date32),
            BuiltinScalarFunction::CurrentTime => Ok(Time64(Nanosecond)),
            BuiltinScalarFunction::Translate => {
                utf8_to_str_type(&input_expr_types[0], "translate")
            }
            BuiltinScalarFunction::Trim => utf8_to_str_type(&input_expr_types[0], "trim"),
            BuiltinScalarFunction::Upper => {
                utf8_to_str_type(&input_expr_types[0], "upper")
            }
            BuiltinScalarFunction::RegexpMatch => Ok(match input_expr_types[0] {
                LargeUtf8 => List(Arc::new(Field::new("item", LargeUtf8, true))),
                Utf8 => List(Arc::new(Field::new("item", Utf8, true))),
                Null => Null,
                _ => {
                    return plan_err!(
                        "The regexp_extract function can only accept strings."
                    );
                }
            }),

            BuiltinScalarFunction::Factorial
            | BuiltinScalarFunction::Gcd
            | BuiltinScalarFunction::Lcm => Ok(Int64),

            BuiltinScalarFunction::Power => match &input_expr_types[0] {
                Int64 => Ok(Int64),
                _ => Ok(Float64),
            },

            BuiltinScalarFunction::Struct => {
                let return_fields = input_expr_types
                    .iter()
                    .enumerate()
                    .map(|(pos, dt)| Field::new(format!("c{pos}"), dt.clone(), true))
                    .collect::<Vec<Field>>();
                Ok(Struct(Fields::from(return_fields)))
            }

            BuiltinScalarFunction::Atan2 => match &input_expr_types[0] {
                Float32 => Ok(Float32),
                _ => Ok(Float64),
            },

            BuiltinScalarFunction::Log => match &input_expr_types[0] {
                Float32 => Ok(Float32),
                _ => Ok(Float64),
            },

            BuiltinScalarFunction::Nanvl => match &input_expr_types[0] {
                Float32 => Ok(Float32),
                _ => Ok(Float64),
            },

            BuiltinScalarFunction::Isnan | BuiltinScalarFunction::Iszero => Ok(Boolean),

            BuiltinScalarFunction::ArrowTypeof => Ok(Utf8),

            BuiltinScalarFunction::Abs => Ok(input_expr_types[0].clone()),

            BuiltinScalarFunction::Acos
            | BuiltinScalarFunction::Asin
            | BuiltinScalarFunction::Atan
            | BuiltinScalarFunction::Acosh
            | BuiltinScalarFunction::Asinh
            | BuiltinScalarFunction::Atanh
            | BuiltinScalarFunction::Ceil
            | BuiltinScalarFunction::Cos
            | BuiltinScalarFunction::Cosh
            | BuiltinScalarFunction::Degrees
            | BuiltinScalarFunction::Exp
            | BuiltinScalarFunction::Floor
            | BuiltinScalarFunction::Ln
            | BuiltinScalarFunction::Log10
            | BuiltinScalarFunction::Log2
            | BuiltinScalarFunction::Radians
            | BuiltinScalarFunction::Round
            | BuiltinScalarFunction::Signum
            | BuiltinScalarFunction::Sin
            | BuiltinScalarFunction::Sinh
            | BuiltinScalarFunction::Sqrt
            | BuiltinScalarFunction::Cbrt
            | BuiltinScalarFunction::Tan
            | BuiltinScalarFunction::Tanh
            | BuiltinScalarFunction::Trunc
            | BuiltinScalarFunction::Cot => match input_expr_types[0] {
                Float32 => Ok(Float32),
                _ => Ok(Float64),
            },
        }
    }

    /// Return the argument [`Signature`] supported by this function
    pub fn signature(&self) -> Signature {
        use DataType::*;
        use IntervalUnit::*;
        use TimeUnit::*;
        use TypeSignature::*;
        // note: the physical expression must accept the type returned by this function or the execution panics.

        // for now, the list is small, as we do not have many built-in functions.
        match self {
            BuiltinScalarFunction::ArrayAppend => Signature::any(2, self.volatility()),
            BuiltinScalarFunction::ArrayPopBack => Signature::any(1, self.volatility()),
            BuiltinScalarFunction::ArrayConcat => {
                Signature::variadic_any(self.volatility())
            }
            BuiltinScalarFunction::ArrayDims => Signature::any(1, self.volatility()),
            BuiltinScalarFunction::ArrayEmpty => Signature::any(1, self.volatility()),
            BuiltinScalarFunction::ArrayElement => Signature::any(2, self.volatility()),
            BuiltinScalarFunction::Flatten => Signature::any(1, self.volatility()),
            BuiltinScalarFunction::ArrayHasAll
            | BuiltinScalarFunction::ArrayHasAny
            | BuiltinScalarFunction::ArrayHas => Signature::any(2, self.volatility()),
            BuiltinScalarFunction::ArrayLength => {
                Signature::variadic_any(self.volatility())
            }
            BuiltinScalarFunction::ArrayNdims => Signature::any(1, self.volatility()),
            BuiltinScalarFunction::ArrayPosition => {
                Signature::variadic_any(self.volatility())
            }
            BuiltinScalarFunction::ArrayPositions => Signature::any(2, self.volatility()),
            BuiltinScalarFunction::ArrayPrepend => Signature::any(2, self.volatility()),
            BuiltinScalarFunction::ArrayRepeat => Signature::any(2, self.volatility()),
            BuiltinScalarFunction::ArrayRemove => Signature::any(2, self.volatility()),
            BuiltinScalarFunction::ArrayRemoveN => Signature::any(3, self.volatility()),
            BuiltinScalarFunction::ArrayRemoveAll => Signature::any(2, self.volatility()),
            BuiltinScalarFunction::ArrayReplace => Signature::any(3, self.volatility()),
            BuiltinScalarFunction::ArrayReplaceN => Signature::any(4, self.volatility()),
            BuiltinScalarFunction::ArrayReplaceAll => {
                Signature::any(3, self.volatility())
            }
            BuiltinScalarFunction::ArraySlice => Signature::any(3, self.volatility()),
            BuiltinScalarFunction::ArrayToString => {
                Signature::variadic_any(self.volatility())
            }
            BuiltinScalarFunction::Cardinality => Signature::any(1, self.volatility()),
            BuiltinScalarFunction::MakeArray => {
                Signature::variadic_any(self.volatility())
            }
            BuiltinScalarFunction::Struct => Signature::variadic(
                struct_expressions::SUPPORTED_STRUCT_TYPES.to_vec(),
                self.volatility(),
            ),
            BuiltinScalarFunction::Concat
            | BuiltinScalarFunction::ConcatWithSeparator => {
                Signature::variadic(vec![Utf8], self.volatility())
            }
            BuiltinScalarFunction::Coalesce => Signature::variadic(
                conditional_expressions::SUPPORTED_COALESCE_TYPES.to_vec(),
                self.volatility(),
            ),
            BuiltinScalarFunction::SHA224
            | BuiltinScalarFunction::SHA256
            | BuiltinScalarFunction::SHA384
            | BuiltinScalarFunction::SHA512
            | BuiltinScalarFunction::MD5 => Signature::uniform(
                1,
                vec![Utf8, LargeUtf8, Binary, LargeBinary],
                self.volatility(),
            ),
            BuiltinScalarFunction::Ascii
            | BuiltinScalarFunction::BitLength
            | BuiltinScalarFunction::CharacterLength
            | BuiltinScalarFunction::InitCap
            | BuiltinScalarFunction::Lower
            | BuiltinScalarFunction::OctetLength
            | BuiltinScalarFunction::Reverse
            | BuiltinScalarFunction::Upper => {
                Signature::uniform(1, vec![Utf8, LargeUtf8], self.volatility())
            }
            BuiltinScalarFunction::Btrim
            | BuiltinScalarFunction::Ltrim
            | BuiltinScalarFunction::Rtrim
            | BuiltinScalarFunction::Trim => Signature::one_of(
                vec![Exact(vec![Utf8]), Exact(vec![Utf8, Utf8])],
                self.volatility(),
            ),
            BuiltinScalarFunction::Chr | BuiltinScalarFunction::ToHex => {
                Signature::uniform(1, vec![Int64], self.volatility())
            }
            BuiltinScalarFunction::Lpad | BuiltinScalarFunction::Rpad => {
                Signature::one_of(
                    vec![
                        Exact(vec![Utf8, Int64]),
                        Exact(vec![LargeUtf8, Int64]),
                        Exact(vec![Utf8, Int64, Utf8]),
                        Exact(vec![LargeUtf8, Int64, Utf8]),
                        Exact(vec![Utf8, Int64, LargeUtf8]),
                        Exact(vec![LargeUtf8, Int64, LargeUtf8]),
                    ],
                    self.volatility(),
                )
            }
            BuiltinScalarFunction::Left
            | BuiltinScalarFunction::Repeat
            | BuiltinScalarFunction::Right => Signature::one_of(
                vec![Exact(vec![Utf8, Int64]), Exact(vec![LargeUtf8, Int64])],
                self.volatility(),
            ),
            BuiltinScalarFunction::ToTimestamp => Signature::uniform(
                1,
                vec![
                    Int64,
                    Timestamp(Nanosecond, None),
                    Timestamp(Microsecond, None),
                    Timestamp(Millisecond, None),
                    Timestamp(Second, None),
                    Utf8,
                ],
                self.volatility(),
            ),
            BuiltinScalarFunction::ToTimestampMillis => Signature::uniform(
                1,
                vec![
                    Int64,
                    Timestamp(Nanosecond, None),
                    Timestamp(Microsecond, None),
                    Timestamp(Millisecond, None),
                    Timestamp(Second, None),
                    Utf8,
                ],
                self.volatility(),
            ),
            BuiltinScalarFunction::ToTimestampMicros => Signature::uniform(
                1,
                vec![
                    Int64,
                    Timestamp(Nanosecond, None),
                    Timestamp(Microsecond, None),
                    Timestamp(Millisecond, None),
                    Timestamp(Second, None),
                    Utf8,
                ],
                self.volatility(),
            ),
            BuiltinScalarFunction::ToTimestampSeconds => Signature::uniform(
                1,
                vec![
                    Int64,
                    Timestamp(Nanosecond, None),
                    Timestamp(Microsecond, None),
                    Timestamp(Millisecond, None),
                    Timestamp(Second, None),
                    Utf8,
                ],
                self.volatility(),
            ),
            BuiltinScalarFunction::FromUnixtime => {
                Signature::uniform(1, vec![Int64], self.volatility())
            }
            BuiltinScalarFunction::Digest => Signature::one_of(
                vec![
                    Exact(vec![Utf8, Utf8]),
                    Exact(vec![LargeUtf8, Utf8]),
                    Exact(vec![Binary, Utf8]),
                    Exact(vec![LargeBinary, Utf8]),
                ],
                self.volatility(),
            ),
            BuiltinScalarFunction::Encode => Signature::one_of(
                vec![
                    Exact(vec![Utf8, Utf8]),
                    Exact(vec![LargeUtf8, Utf8]),
                    Exact(vec![Binary, Utf8]),
                    Exact(vec![LargeBinary, Utf8]),
                ],
                self.volatility(),
            ),
            BuiltinScalarFunction::Decode => Signature::one_of(
                vec![
                    Exact(vec![Utf8, Utf8]),
                    Exact(vec![LargeUtf8, Utf8]),
                    Exact(vec![Binary, Utf8]),
                    Exact(vec![LargeBinary, Utf8]),
                ],
                self.volatility(),
            ),
            BuiltinScalarFunction::DateTrunc => Signature::one_of(
                vec![
                    Exact(vec![Utf8, Timestamp(Nanosecond, None)]),
                    Exact(vec![Utf8, Timestamp(Microsecond, None)]),
                    Exact(vec![Utf8, Timestamp(Millisecond, None)]),
                    Exact(vec![Utf8, Timestamp(Second, None)]),
                ],
                self.volatility(),
            ),
            BuiltinScalarFunction::DateBin => {
                let base_sig = |array_type: TimeUnit| {
                    vec![
                        Exact(vec![
                            Interval(MonthDayNano),
                            Timestamp(array_type.clone(), None),
                            Timestamp(Nanosecond, None),
                        ]),
                        Exact(vec![
                            Interval(DayTime),
                            Timestamp(array_type.clone(), None),
                            Timestamp(Nanosecond, None),
                        ]),
                        Exact(vec![
                            Interval(MonthDayNano),
                            Timestamp(array_type.clone(), None),
                        ]),
                        Exact(vec![Interval(DayTime), Timestamp(array_type, None)]),
                    ]
                };

                let full_sig = [Nanosecond, Microsecond, Millisecond, Second]
                    .into_iter()
                    .map(base_sig)
                    .collect::<Vec<_>>()
                    .concat();

                Signature::one_of(full_sig, self.volatility())
            }
            BuiltinScalarFunction::DatePart => Signature::one_of(
                vec![
                    Exact(vec![Utf8, Date32]),
                    Exact(vec![Utf8, Date64]),
                    Exact(vec![Utf8, Timestamp(Second, None)]),
                    Exact(vec![Utf8, Timestamp(Microsecond, None)]),
                    Exact(vec![Utf8, Timestamp(Millisecond, None)]),
                    Exact(vec![Utf8, Timestamp(Nanosecond, None)]),
                    Exact(vec![Utf8, Timestamp(Nanosecond, Some("+00:00".into()))]),
                ],
                self.volatility(),
            ),
            BuiltinScalarFunction::SplitPart => Signature::one_of(
                vec![
                    Exact(vec![Utf8, Utf8, Int64]),
                    Exact(vec![LargeUtf8, Utf8, Int64]),
                    Exact(vec![Utf8, LargeUtf8, Int64]),
                    Exact(vec![LargeUtf8, LargeUtf8, Int64]),
                ],
                self.volatility(),
            ),
            BuiltinScalarFunction::StringToArray => Signature::one_of(
                vec![
                    TypeSignature::Uniform(2, vec![Utf8, LargeUtf8]),
                    TypeSignature::Uniform(3, vec![Utf8, LargeUtf8]),
                ],
                self.volatility(),
            ),
            BuiltinScalarFunction::Strpos | BuiltinScalarFunction::StartsWith => {
                Signature::one_of(
                    vec![
                        Exact(vec![Utf8, Utf8]),
                        Exact(vec![Utf8, LargeUtf8]),
                        Exact(vec![LargeUtf8, Utf8]),
                        Exact(vec![LargeUtf8, LargeUtf8]),
                    ],
                    self.volatility(),
                )
            }

            BuiltinScalarFunction::Substr => Signature::one_of(
                vec![
                    Exact(vec![Utf8, Int64]),
                    Exact(vec![LargeUtf8, Int64]),
                    Exact(vec![Utf8, Int64, Int64]),
                    Exact(vec![LargeUtf8, Int64, Int64]),
                ],
                self.volatility(),
            ),

            BuiltinScalarFunction::Replace | BuiltinScalarFunction::Translate => {
                Signature::one_of(vec![Exact(vec![Utf8, Utf8, Utf8])], self.volatility())
            }
            BuiltinScalarFunction::RegexpReplace => Signature::one_of(
                vec![
                    Exact(vec![Utf8, Utf8, Utf8]),
                    Exact(vec![Utf8, Utf8, Utf8, Utf8]),
                ],
                self.volatility(),
            ),

            BuiltinScalarFunction::NullIf => {
                Signature::uniform(2, SUPPORTED_NULLIF_TYPES.to_vec(), self.volatility())
            }
            BuiltinScalarFunction::RegexpMatch => Signature::one_of(
                vec![
                    Exact(vec![Utf8, Utf8]),
                    Exact(vec![LargeUtf8, Utf8]),
                    Exact(vec![Utf8, Utf8, Utf8]),
                    Exact(vec![LargeUtf8, Utf8, Utf8]),
                ],
                self.volatility(),
            ),
            BuiltinScalarFunction::Pi => Signature::exact(vec![], self.volatility()),
            BuiltinScalarFunction::Random => Signature::exact(vec![], self.volatility()),
            BuiltinScalarFunction::Uuid => Signature::exact(vec![], self.volatility()),
            BuiltinScalarFunction::Power => Signature::one_of(
                vec![Exact(vec![Int64, Int64]), Exact(vec![Float64, Float64])],
                self.volatility(),
            ),
            BuiltinScalarFunction::Round => Signature::one_of(
                vec![
                    Exact(vec![Float64, Int64]),
                    Exact(vec![Float32, Int64]),
                    Exact(vec![Float64]),
                    Exact(vec![Float32]),
                ],
                self.volatility(),
            ),
            BuiltinScalarFunction::Trunc => Signature::one_of(
                vec![
                    Exact(vec![Float32, Int64]),
                    Exact(vec![Float64, Int64]),
                    Exact(vec![Float64]),
                    Exact(vec![Float32]),
                ],
                self.volatility(),
            ),
            BuiltinScalarFunction::Atan2 => Signature::one_of(
                vec![Exact(vec![Float32, Float32]), Exact(vec![Float64, Float64])],
                self.volatility(),
            ),
            BuiltinScalarFunction::Log => Signature::one_of(
                vec![
                    Exact(vec![Float32]),
                    Exact(vec![Float64]),
                    Exact(vec![Float32, Float32]),
                    Exact(vec![Float64, Float64]),
                ],
                self.volatility(),
            ),
            BuiltinScalarFunction::Nanvl => Signature::one_of(
                vec![Exact(vec![Float32, Float32]), Exact(vec![Float64, Float64])],
                self.volatility(),
            ),
            BuiltinScalarFunction::Factorial => {
                Signature::uniform(1, vec![Int64], self.volatility())
            }
            BuiltinScalarFunction::Gcd | BuiltinScalarFunction::Lcm => {
                Signature::uniform(2, vec![Int64], self.volatility())
            }
            BuiltinScalarFunction::ArrowTypeof => Signature::any(1, self.volatility()),
            BuiltinScalarFunction::Abs => Signature::any(1, self.volatility()),

            BuiltinScalarFunction::Acos
            | BuiltinScalarFunction::Asin
            | BuiltinScalarFunction::Atan
            | BuiltinScalarFunction::Acosh
            | BuiltinScalarFunction::Asinh
            | BuiltinScalarFunction::Atanh
            | BuiltinScalarFunction::Cbrt
            | BuiltinScalarFunction::Ceil
            | BuiltinScalarFunction::Cos
            | BuiltinScalarFunction::Cosh
            | BuiltinScalarFunction::Degrees
            | BuiltinScalarFunction::Exp
            | BuiltinScalarFunction::Floor
            | BuiltinScalarFunction::Ln
            | BuiltinScalarFunction::Log10
            | BuiltinScalarFunction::Log2
            | BuiltinScalarFunction::Radians
            | BuiltinScalarFunction::Signum
            | BuiltinScalarFunction::Sin
            | BuiltinScalarFunction::Sinh
            | BuiltinScalarFunction::Sqrt
            | BuiltinScalarFunction::Tan
            | BuiltinScalarFunction::Tanh
            | BuiltinScalarFunction::Cot => {
                // math expressions expect 1 argument of type f64 or f32
                // priority is given to f64 because e.g. `sqrt(1i32)` is in IR (real numbers) and thus we
                // return the best approximation for it (in f64).
                // We accept f32 because in this case it is clear that the best approximation
                // will be as good as the number of digits in the number
                Signature::uniform(1, vec![Float64, Float32], self.volatility())
            }
            BuiltinScalarFunction::Now
            | BuiltinScalarFunction::CurrentDate
            | BuiltinScalarFunction::CurrentTime => {
                Signature::uniform(0, vec![], self.volatility())
            }
            BuiltinScalarFunction::Isnan | BuiltinScalarFunction::Iszero => {
                Signature::one_of(
                    vec![Exact(vec![Float32]), Exact(vec![Float64])],
                    self.volatility(),
                )
            }
        }
    }
}

fn aliases(func: &BuiltinScalarFunction) -> &'static [&'static str] {
    match func {
        BuiltinScalarFunction::Abs => &["abs"],
        BuiltinScalarFunction::Acos => &["acos"],
        BuiltinScalarFunction::Acosh => &["acosh"],
        BuiltinScalarFunction::Asin => &["asin"],
        BuiltinScalarFunction::Asinh => &["asinh"],
        BuiltinScalarFunction::Atan => &["atan"],
        BuiltinScalarFunction::Atanh => &["atanh"],
        BuiltinScalarFunction::Atan2 => &["atan2"],
        BuiltinScalarFunction::Cbrt => &["cbrt"],
        BuiltinScalarFunction::Ceil => &["ceil"],
        BuiltinScalarFunction::Cos => &["cos"],
        BuiltinScalarFunction::Cot => &["cot"],
        BuiltinScalarFunction::Cosh => &["cosh"],
        BuiltinScalarFunction::Degrees => &["degrees"],
        BuiltinScalarFunction::Exp => &["exp"],
        BuiltinScalarFunction::Factorial => &["factorial"],
        BuiltinScalarFunction::Floor => &["floor"],
        BuiltinScalarFunction::Gcd => &["gcd"],
        BuiltinScalarFunction::Isnan => &["isnan"],
        BuiltinScalarFunction::Iszero => &["iszero"],
        BuiltinScalarFunction::Lcm => &["lcm"],
        BuiltinScalarFunction::Ln => &["ln"],
        BuiltinScalarFunction::Log => &["log"],
        BuiltinScalarFunction::Log10 => &["log10"],
        BuiltinScalarFunction::Log2 => &["log2"],
        BuiltinScalarFunction::Nanvl => &["nanvl"],
        BuiltinScalarFunction::Pi => &["pi"],
        BuiltinScalarFunction::Power => &["power", "pow"],
        BuiltinScalarFunction::Radians => &["radians"],
        BuiltinScalarFunction::Random => &["random"],
        BuiltinScalarFunction::Round => &["round"],
        BuiltinScalarFunction::Signum => &["signum"],
        BuiltinScalarFunction::Sin => &["sin"],
        BuiltinScalarFunction::Sinh => &["sinh"],
        BuiltinScalarFunction::Sqrt => &["sqrt"],
        BuiltinScalarFunction::Tan => &["tan"],
        BuiltinScalarFunction::Tanh => &["tanh"],
        BuiltinScalarFunction::Trunc => &["trunc"],

        // conditional functions
        BuiltinScalarFunction::Coalesce => &["coalesce"],
        BuiltinScalarFunction::NullIf => &["nullif"],

        // string functions
        BuiltinScalarFunction::Ascii => &["ascii"],
        BuiltinScalarFunction::BitLength => &["bit_length"],
        BuiltinScalarFunction::Btrim => &["btrim"],
        BuiltinScalarFunction::CharacterLength => {
            &["character_length", "char_length", "length"]
        }
        BuiltinScalarFunction::Concat => &["concat"],
        BuiltinScalarFunction::ConcatWithSeparator => &["concat_ws"],
        BuiltinScalarFunction::Chr => &["chr"],
        BuiltinScalarFunction::InitCap => &["initcap"],
        BuiltinScalarFunction::Left => &["left"],
        BuiltinScalarFunction::Lower => &["lower"],
        BuiltinScalarFunction::Lpad => &["lpad"],
        BuiltinScalarFunction::Ltrim => &["ltrim"],
        BuiltinScalarFunction::OctetLength => &["octet_length"],
        BuiltinScalarFunction::Repeat => &["repeat"],
        BuiltinScalarFunction::Replace => &["replace"],
        BuiltinScalarFunction::Reverse => &["reverse"],
        BuiltinScalarFunction::Right => &["right"],
        BuiltinScalarFunction::Rpad => &["rpad"],
        BuiltinScalarFunction::Rtrim => &["rtrim"],
        BuiltinScalarFunction::SplitPart => &["split_part"],
        BuiltinScalarFunction::StringToArray => &["string_to_array", "string_to_list"],
        BuiltinScalarFunction::StartsWith => &["starts_with"],
        BuiltinScalarFunction::Strpos => &["strpos"],
        BuiltinScalarFunction::Substr => &["substr"],
        BuiltinScalarFunction::ToHex => &["to_hex"],
        BuiltinScalarFunction::Translate => &["translate"],
        BuiltinScalarFunction::Trim => &["trim"],
        BuiltinScalarFunction::Upper => &["upper"],
        BuiltinScalarFunction::Uuid => &["uuid"],

        // regex functions
        BuiltinScalarFunction::RegexpMatch => &["regexp_match"],
        BuiltinScalarFunction::RegexpReplace => &["regexp_replace"],

        // time/date functions
        BuiltinScalarFunction::Now => &["now"],
        BuiltinScalarFunction::CurrentDate => &["current_date"],
        BuiltinScalarFunction::CurrentTime => &["current_time"],
        BuiltinScalarFunction::DateBin => &["date_bin"],
        BuiltinScalarFunction::DateTrunc => &["date_trunc", "datetrunc"],
        BuiltinScalarFunction::DatePart => &["date_part", "datepart"],
        BuiltinScalarFunction::ToTimestamp => &["to_timestamp"],
        BuiltinScalarFunction::ToTimestampMillis => &["to_timestamp_millis"],
        BuiltinScalarFunction::ToTimestampMicros => &["to_timestamp_micros"],
        BuiltinScalarFunction::ToTimestampSeconds => &["to_timestamp_seconds"],
        BuiltinScalarFunction::FromUnixtime => &["from_unixtime"],

        // hashing functions
        BuiltinScalarFunction::Digest => &["digest"],
        BuiltinScalarFunction::MD5 => &["md5"],
        BuiltinScalarFunction::SHA224 => &["sha224"],
        BuiltinScalarFunction::SHA256 => &["sha256"],
        BuiltinScalarFunction::SHA384 => &["sha384"],
        BuiltinScalarFunction::SHA512 => &["sha512"],

        // encode/decode
        BuiltinScalarFunction::Encode => &["encode"],
        BuiltinScalarFunction::Decode => &["decode"],

        // other functions
        BuiltinScalarFunction::ArrowTypeof => &["arrow_typeof"],

        // array functions
        BuiltinScalarFunction::ArrayAppend => &[
            "array_append",
            "list_append",
            "array_push_back",
            "list_push_back",
        ],
        BuiltinScalarFunction::ArrayConcat => {
            &["array_concat", "array_cat", "list_concat", "list_cat"]
        }
        BuiltinScalarFunction::ArrayDims => &["array_dims", "list_dims"],
        BuiltinScalarFunction::ArrayEmpty => &["empty"],
        BuiltinScalarFunction::ArrayElement => &[
            "array_element",
            "array_extract",
            "list_element",
            "list_extract",
        ],
        BuiltinScalarFunction::Flatten => &["flatten"],
        BuiltinScalarFunction::ArrayHasAll => &["array_has_all", "list_has_all"],
        BuiltinScalarFunction::ArrayHasAny => &["array_has_any", "list_has_any"],
        BuiltinScalarFunction::ArrayHas => {
            &["array_has", "list_has", "array_contains", "list_contains"]
        }
        BuiltinScalarFunction::ArrayLength => &["array_length", "list_length"],
        BuiltinScalarFunction::ArrayNdims => &["array_ndims", "list_ndims"],
        BuiltinScalarFunction::ArrayPopBack => &["array_pop_back", "list_pop_back"],
        BuiltinScalarFunction::ArrayPosition => &[
            "array_position",
            "list_position",
            "array_indexof",
            "list_indexof",
        ],
        BuiltinScalarFunction::ArrayPositions => &["array_positions", "list_positions"],
        BuiltinScalarFunction::ArrayPrepend => &[
            "array_prepend",
            "list_prepend",
            "array_push_front",
            "list_push_front",
        ],
        BuiltinScalarFunction::ArrayRepeat => &["array_repeat", "list_repeat"],
        BuiltinScalarFunction::ArrayRemove => &["array_remove", "list_remove"],
        BuiltinScalarFunction::ArrayRemoveN => &["array_remove_n", "list_remove_n"],
        BuiltinScalarFunction::ArrayRemoveAll => &["array_remove_all", "list_remove_all"],
        BuiltinScalarFunction::ArrayReplace => &["array_replace", "list_replace"],
        BuiltinScalarFunction::ArrayReplaceN => &["array_replace_n", "list_replace_n"],
        BuiltinScalarFunction::ArrayReplaceAll => {
            &["array_replace_all", "list_replace_all"]
        }
        BuiltinScalarFunction::ArraySlice => &["array_slice", "list_slice"],
        BuiltinScalarFunction::ArrayToString => &[
            "array_to_string",
            "list_to_string",
            "array_join",
            "list_join",
        ],
        BuiltinScalarFunction::Cardinality => &["cardinality"],
        BuiltinScalarFunction::MakeArray => &["make_array", "make_list"],

        // struct functions
        BuiltinScalarFunction::Struct => &["struct"],
    }
}

impl fmt::Display for BuiltinScalarFunction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // .unwrap is safe here because compiler makes sure the map will have matches for each BuiltinScalarFunction
        write!(f, "{}", function_to_name().get(self).unwrap())
    }
}

impl FromStr for BuiltinScalarFunction {
    type Err = DataFusionError;
    fn from_str(name: &str) -> Result<BuiltinScalarFunction> {
        if let Some(func) = name_to_function().get(name) {
            Ok(*func)
        } else {
            plan_err!("There is no built-in function named {name}")
        }
    }
}

macro_rules! make_utf8_to_return_type {
    ($FUNC:ident, $largeUtf8Type:expr, $utf8Type:expr) => {
        fn $FUNC(arg_type: &DataType, name: &str) -> Result<DataType> {
            Ok(match arg_type {
                DataType::LargeUtf8 => $largeUtf8Type,
                DataType::Utf8 => $utf8Type,
                DataType::Null => DataType::Null,
                DataType::Dictionary(_, value_type) => match **value_type {
                    DataType::LargeUtf8 => $largeUtf8Type,
                    DataType::Utf8 => $utf8Type,
                    DataType::Null => DataType::Null,
                    _ => {
                        return plan_err!(
                            "The {:?} function can only accept strings, but got {:?}.",
                            name,
                            **value_type
                        );
                    }
                },
                data_type => {
                    return plan_err!(
                        "The {:?} function can only accept strings, but got {:?}.",
                        name,
                        data_type
                    );
                }
            })
        }
    };
}

make_utf8_to_return_type!(utf8_to_str_type, DataType::LargeUtf8, DataType::Utf8);
make_utf8_to_return_type!(utf8_to_int_type, DataType::Int64, DataType::Int32);

fn utf8_or_binary_to_binary_type(arg_type: &DataType, name: &str) -> Result<DataType> {
    Ok(match arg_type {
        DataType::LargeUtf8
        | DataType::Utf8
        | DataType::Binary
        | DataType::LargeBinary => DataType::Binary,
        DataType::Null => DataType::Null,
        _ => {
            return plan_err!(
                "The {name:?} function can only accept strings or binary arrays."
            );
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // Test for BuiltinScalarFunction's Display and from_str() implementations.
    // For each variant in BuiltinScalarFunction, it converts the variant to a string
    // and then back to a variant. The test asserts that the original variant and
    // the reconstructed variant are the same.
    fn test_display_and_from_str() {
        for (_, func_original) in name_to_function().iter() {
            let func_name = func_original.to_string();
            let func_from_str = BuiltinScalarFunction::from_str(&func_name).unwrap();
            assert_eq!(func_from_str, *func_original);
        }
    }
}
