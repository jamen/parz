//! Minimal parser combinators
//!
//! ## Basic parsers
//!
//! | Items | Description | Example |
//! |---|---|---|
//! | [`and`] | Combine two parsers where both must succeed. | `and(u16l, u32l)` |
//! | [`or`] | Combine two parsers where at least one must succeed. | `or(u16l, u32l)` |
//! | [`take`] | Take N bytes. | `take(42)` |
//! | [`seq`] | Run a parser N times in sequence. | `seq(u32l, 42)` |
//! | [`tag`] | Match a sequence of bytes. | `tag("hello")` |
//! | [`opt`] | Allow a parser to fail. | `opt(tag("hello"))` |
//! | [`pod`] | Transmute bytes into a type. **Requires the `bytemuck` feature** | `seq(pod::<MyType>, 4)` |
//! | [`finish`] | Ensure there is no bytes left | `finish(seq(u16l, 128))` |
//!
//! ## Number parsers
//!
//! | | `u8` | `u16` | `u32` | `u64` | `u128` | `f32` | `f64` |
//! |---|---|---|---|---|---|---|---|
//! | **Little Endian** | [`byte`] | [`u16l`] | [`u32l`] | [`u64l`] | [`u128l`] | [`f32l`] | [`f64l`] |
//! | **Big Endian** | [`byte`] | [`u16b`] | [`u32b`] | [`u64b`] | [`u128b`] | [`f32b`] | [`f64b`] |
//!
//! ## Features
//!
//! - `bytemuck`: Enables the [`pod`] parser
//! ## MSRV
//!
//! Minimum supported Rust version is: 1.60
//!

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};

pub type Step<'a, Output, Error> = (&'a [u8], Result<Output, Error>);

pub struct ByteError;

pub fn byte<'a, Error: From<ByteError>>(input: &'a [u8]) -> Step<'a, u8, Error> {
    match input.split_first() {
        Some((&byte, rest)) => (rest, Ok(byte)),
        None => (input, Err(ByteError.into())),
    }
}

pub struct TakeError<'a>(
    /// Where the error happened
    pub &'a [u8],
);

pub fn take<'a, Error: From<TakeError<'a>>>(
    count: usize,
) -> impl Fn(&'a [u8]) -> Step<'a, &'a [u8], Error> {
    move |input| {
        let (out, input) = input.split_at(count);
        match out.len() {
            0 if count != 0 => (input, Err(TakeError(input).into())),
            _ => (input, Ok(out)),
        }
    }
}

pub struct SeqError<'a, ChildError> {
    /// Where the error happened
    pub at: &'a [u8],
    /// What iteration the error happened
    pub step: usize,
    /// The child parser's error
    pub child_error: ChildError,
}

impl<'a, ChildError: Debug> Debug for SeqError<'a, ChildError> {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("SeqError")
            .field("step", &self.step)
            .field("child_error", &self.child_error)
            // .field("input", &InputDebug(self.input))
            .finish()
    }
}

pub fn seq<'a, Output, Error: From<SeqError<'a, ChildError>>, ChildError>(
    count: usize,
    child: impl Fn(&'a [u8]) -> Step<'a, Output, ChildError>,
) -> impl Fn(&'a [u8]) -> Step<'a, Vec<Output>, Error> {
    move |mut input| {
        let before = input;
        let mut out = Vec::with_capacity(count);
        for step in 0..count {
            let (rest, result) = (child)(input);
            match result {
                Ok(x) => out.push(x),
                Err(child_error) => {
                    return (
                        before,
                        Err(SeqError {
                            at: input,
                            step,
                            child_error,
                        }
                        .into()),
                    )
                }
            }
            input = rest;
        }
        (input, Ok(out))
    }
}

pub enum OptError {}

pub fn opt<'a, Output, Error, Parser>(
    child: impl Fn(&'a [u8]) -> Step<'a, Output, Error>,
) -> impl Fn(&'a [u8]) -> Step<'a, Option<Output>, OptError> {
    move |input| {
        let (rest, result) = (child)(input);
        match result {
            Ok(x) => (rest, Ok(Some(x))),
            Err(_) => (input, Ok(None)),
        }
    }
}

pub struct FinishError<'a>(
    /// Where the error happened
    pub &'a [u8],
);

pub fn finish<'a, Output, Error: From<ChildError> + From<FinishError<'a>>, ChildError>(
    child: impl Fn(&'a [u8]) -> Step<'a, Output, ChildError>,
) -> impl Fn(&'a [u8]) -> Step<'a, Output, Error> {
    move |input| {
        let (rest, result) = (child)(input);
        match result {
            Ok(x) => match input.len() {
                0 => (rest, Ok(x)),
                _ => (input, Err(FinishError(input).into())),
            },
            Err(e) => (input, Err(e.into())),
        }
    }
}

pub struct TagError<'a>(
    /// Where the error happened
    pub &'a [u8],
);

impl<'a> From<TakeError<'a>> for TagError<'a> {
    fn from(x: TakeError<'a>) -> Self {
        Self(x.0)
    }
}

pub fn tag<'a, 'b, Error: From<TagError<'a>>>(
    key: &'b [u8],
) -> impl Fn(&'a [u8]) -> Step<'a, &'a [u8], Error> + 'b {
    move |input| match take::<TagError>(key.len())(input) {
        (rest, Ok(result)) if result == key => (rest, Ok(result)),
        (_, Err(x)) => (input, Err(x.into())),
        _ => (input, Err(TagError(input).into())),
    }
}

pub fn or<'a, Output1, Output2, Error: From<Error2>, Error1, Error2>(
    one: impl Fn(&'a [u8]) -> Step<'a, Output1, Error1>,
    two: impl Fn(&'a [u8]) -> Step<'a, Output2, Error2>,
) -> impl Fn(&'a [u8]) -> Step<'a, (Option<Output1>, Option<Output2>), Error> {
    move |input| {
        let (rest, result1) = (one)(input);
        let input = if result1.is_ok() { rest } else { input };
        let (rest, result2) = (two)(input);
        (rest, Ok((result1.ok(), result2.ok())))
    }
}

pub fn and<'a, Output1, Output2, Error: From<Error1> + From<Error2>, Error1, Error2>(
    one: impl Fn(&'a [u8]) -> Step<'a, Output1, Error1>,
    two: impl Fn(&'a [u8]) -> Step<'a, Output2, Error2>,
) -> impl Fn(&'a [u8]) -> Step<'a, (Output1, Output2), Error> {
    move |input| {
        let before = input;
        match (one)(input) {
            (rest, Ok(x)) => match (two)(rest) {
                (rest, Ok(y)) => (rest, Ok((x, y))),
                (_, Err(e)) => (before, Err(e.into())),
            },
            (_, Err(e)) => (before, Err(e.into())),
        }
    }
}

#[cfg(feature = "bytemuck")]
use bytemuck::{Pod, PodCastError};

#[cfg(feature = "bytemuck")]
pub struct PodError<'a> {
    /// Where the error happened
    pub at: &'a [u8],
    pub pod_error: PodCastError,
}

#[cfg(feature = "bytemuck")]
pub fn pod<'a, Output: Pod, Error: From<PodError<'a>>>(
    input: &'a [u8],
) -> Step<'a, &'a Output, Error> {
    let (rest, bytes) = input.split_at(core::mem::size_of::<Output>());
    match bytemuck::try_from_bytes(bytes) {
        Ok(x) => (rest, Ok(x)),
        Err(pod_error) => (
            input,
            Err(PodError {
                at: input,
                pod_error,
            }
            .into()),
        ),
    }
}

macro_rules! num_impl {
    (
        $(#[$m:meta])*
        $num_ty:ty, $endian_fn:ident, $fn_name:ident, $err_name:ident;
        $($rest:tt)*
    ) => {
        pub struct $err_name<'a>(&'a [u8]);

        $(#[$m])*
        pub fn $fn_name<'a, Error: From<$err_name<'a>>>(
            input: &'a [u8]
        ) -> Step<'a, $num_ty, $err_name> {
            let (out, rest) = input.split_at(core::mem::size_of::<$num_ty>());
            let out = match out.try_into() {
                Ok(x) => x,
                Err(_) => return (input, Err($err_name(input).into())),
            };
            (rest, Ok(<$num_ty>::$endian_fn(out)))
        }

        num_impl! { $($rest)* }
    };
    () => {}
}

num_impl! {
    /// Parse unsigned 16-bit little-endian integer.
    u16, from_le_bytes, u16l, U16LError;
    /// Parse signed 16-bit little-endian integer.
    i16, from_le_bytes, i16l, I16LError;
    /// Parse unsigned 16-bit big-endian integer.
    u16, from_be_bytes, u16b, U16BError;
    /// Parse signed 16-bit big-endian integer.
    i16, from_be_bytes, i16b, I16BError;

    /// Parse unsigned 32-bit little-endian integer.
    u32, from_le_bytes, u32l, U32LError;
    /// Parse signed 32-bit little-endian integer.
    i32, from_le_bytes, i32l, I32LError;
    /// Parse unsigned 32-bit big-endian integer.
    u32, from_be_bytes, u32b, U32BError;
    /// Parse signed 32-bit big-endian integer.
    i32, from_be_bytes, i32b, I32BError;

    /// Parse unsigned 64-bit little-endian integer.
    u64, from_le_bytes, u64l, U64LError;
    /// Parse signed 64-bit little-endian integer.
    i64, from_le_bytes, i64l, I64LError;
    /// Parse unsigned 64-bit big-endian integer.
    u64, from_be_bytes, u64b, U64BError;
    /// Parse signed 64-bit big-endian integer.
    i64, from_be_bytes, i64b, I64BError;

    /// Parse unsigned 128-bit little-endian integer.
    u128, from_le_bytes, u128l, U128LError;
    /// Parse signed 128-bit little-endian integer.
    i128, from_le_bytes, i128l, I128LError;
    /// Parse unsigned 128-bit big-endian integer.
    u128, from_be_bytes, u128b, U128BError;
    /// Parse signed 128-bit big-endian integer.
    i128, from_be_bytes, i128b, I128BError;

    /// Parse 32-bit little-endian float.
    f32, from_le_bytes, f32l, F32LError;
    /// Parse 32-bit big-endian float.
    f32, from_be_bytes, f32b, F32BError;

    /// Parse 64-bit little-endian float.
    f64, from_le_bytes, f64l, F64LError;
    /// Parse 64-bit big-endian float.
    f64, from_be_bytes, f64b, F64BError;
}
