#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};

use either::Either;

pub struct ParseResult<'a, Output, Error>(&'a [u8], Result<Output, Error>);

pub struct TakeError<'a>(
    /// Where the error happened
    pub &'a [u8],
);

pub fn take<'a, Error: From<TakeError<'a>>>(
    count: usize,
) -> impl Fn(&'a [u8]) -> (&'a [u8], Result<&'a [u8], Error>) {
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
    child: impl Fn(&'a [u8]) -> (&'a [u8], Result<Output, ChildError>),
) -> impl Fn(&'a [u8]) -> (&'a [u8], Result<Vec<Output>, Error>) {
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

pub fn opt<'a, Output, Error>(
    child: impl Fn(&'a [u8]) -> (&'a [u8], Result<Output, Error>),
) -> impl Fn(&'a [u8]) -> (&'a [u8], Result<Option<Output>, OptError>) {
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
    child: impl Fn(&'a [u8]) -> (&'a [u8], Result<Output, ChildError>),
) -> impl Fn(&'a [u8]) -> (&'a [u8], Result<Output, Error>) {
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
) -> impl Fn(&'a [u8]) -> (&'a [u8], Result<&'a [u8], Error>) + 'b {
    move |input| match take::<TagError>(key.len())(input) {
        (rest, Ok(result)) if result == key => (rest, Ok(result)),
        (_, Err(x)) => (input, Err(x.into())),
        _ => (input, Err(TagError(input).into())),
    }
}

pub fn or<
    'a,
    FirstChildOutput,
    SecondChildOutput,
    Error: From<SecondChildError>,
    FirstChildError,
    SecondChildError,
>(
    first_child: impl Fn(&'a [u8]) -> (&'a [u8], Result<FirstChildOutput, FirstChildError>),
    second_child: impl Fn(&'a [u8]) -> (&'a [u8], Result<SecondChildOutput, SecondChildError>),
) -> impl Fn(
    &'a [u8],
) -> (
    &'a [u8],
    Result<Either<FirstChildOutput, SecondChildOutput>, Error>,
) {
    move |input| match (first_child)(input) {
        (rest, Ok(x)) => (rest, Ok(Either::Left(x))),
        _ => match (second_child)(input) {
            (rest, Ok(x)) => (rest, Ok(Either::Right(x))),
            (rest, Err(e)) => (rest, Err(e.into())),
        },
    }
}

pub fn and<
    'a,
    FirstChildOutput,
    SecondChildOutput,
    Error: From<FirstChildError> + From<SecondChildError>,
    FirstChildError,
    SecondChildError,
>(
    first_child: impl Fn(&'a [u8]) -> (&'a [u8], Result<FirstChildOutput, FirstChildError>),
    second_child: impl Fn(&'a [u8]) -> (&'a [u8], Result<SecondChildOutput, SecondChildError>),
) -> impl Fn(
    &'a [u8],
) -> (
    &'a [u8],
    Result<(FirstChildOutput, SecondChildOutput), Error>,
) {
    move |input| {
        let before = input;
        match (first_child)(input) {
            (rest, Ok(x)) => match (second_child)(rest) {
                (rest, Ok(y)) => (rest, Ok((x, y))),
                (_, Err(e)) => (before, Err(e.into())),
            },
            (_, Err(e)) => (before, Err(e.into())),
        }
    }
}
