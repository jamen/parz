#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};

use either::Either;

pub type Step<'a, Output, Error> = (&'a [u8], Result<Output, Error>);

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

// pub fn seq_array<const N: usize>

// TODO: Replace with `!` type when it stablizes
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

#[derive(Debug)]
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

#[derive(Debug)]
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
) -> impl Fn(&'a [u8]) -> Step<'a, Either<Output1, Output2>, Error> {
    move |input| match (one)(input) {
        (rest, Ok(x)) => (rest, Ok(Either::Left(x))),
        _ => match (two)(input) {
            (rest, Ok(x)) => (rest, Ok(Either::Right(x))),
            (rest, Err(e)) => (rest, Err(e.into())),
        },
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