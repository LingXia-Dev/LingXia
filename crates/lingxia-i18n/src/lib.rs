//! Message catalogues for lxapps, the View and packages: FormatJS JSON files whose
//! `defaultMessage` is an ICU MessageFormat 1 message, and the checks a translation must pass.

pub mod catalogue;
pub mod dts;
pub mod format;
pub mod js;
pub mod message;
pub mod pseudo;

pub use catalogue::{Catalogue, Entry, Problem, check, valid_key};
pub use message::{
    Arg, DateStyle, Message, NumberStyle, ParseError, Part, PluralCategory, Selector, Signature,
    Use, parse, signature_drift,
};
