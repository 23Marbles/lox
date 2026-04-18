//! Error codes for the tree-walk interpreter.

use core::{error::Error, fmt::Display, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
#[allow(missing_docs)]
pub enum ErrorCode {
    CoreErrorFromString = 1,
    UnexpectedCharacter = 2,
    UnterminatedString = 3,
    #[cfg(feature = "inline_comments")]
    UnterminatedComment = 4,
    UnexpectedTokenKind = 5,
    ExpectedTokenKind = 6,
    InvalidAssignTarget = 7,
    Over255Arguments = 8,
    ExpectedName = 9,
    ExpectedNumber = 10,
    ReadInOwnInitilaizer = 11,
    DefinedMultipleTimes = 12,
    ReturnFromTop = 13,
    ReturnValueFromInit = 14,
    ThisOutsideClass = 15,
    SelfReferentialInheritance = 16,
    SuperOutsideSubclass = 17,
    SuperClassIsNotAClass = 19,
    StackOverflow = 20,
    IncompatableUnaryOperator = 21,
    IncompatableBinaryOperator = 22,
    UndeclaredVariable = 23,
    NonCallable = 24,
    IncorrectArity = 25,
    IncompatibleConversion = 26,
    #[cfg(feature = "div_zero_error")]
    DivisionByZero = 27,
    NonExistentField = 28,
    CannotGetProperties = 29,
    UninitializedVariable = 30,
    IncorrectArgType = 31,
    CouldNotConvert = 32,
    OutOfBounds = 34,
    NonIntegerIndex = 35,
    NonPositiveIntegerOperand = 36,
    IncorrectType = 37,
    CannotIndexInto = 38,
    IncorrectIndexKind = 39,
    CannotSetProperties = 40,
    PopEmptyList = 41,
    AssertionFailed = 254,
    UserForcedError = 255,
}

impl Display for ErrorCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "E{:05}", *self as u8)
    }
}

#[derive(Debug, Clone)]
/// Error code from string parsing error.
pub enum ErrorCodeFromStrErr {
    /// Error parsing the error code as a u8.
    U8ParseError(core::num::ParseIntError),
    /// Invalid discriminant value.
    InvalidDiscriminant,
    /// Invalid format.
    InvalidFormat,
}

impl From<core::num::ParseIntError> for ErrorCodeFromStrErr {
    fn from(err: core::num::ParseIntError) -> Self {
        ErrorCodeFromStrErr::U8ParseError(err)
    }
}

impl Display for ErrorCodeFromStrErr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ErrorCodeFromStrErr::U8ParseError(err) => write!(f, "{}", err),
            ErrorCodeFromStrErr::InvalidDiscriminant => write!(f, "invalid discriminant"),
            ErrorCodeFromStrErr::InvalidFormat => write!(f, "invalid format"),
        }
    }
}

impl Error for ErrorCodeFromStrErr {}

impl TryFrom<u8> for ErrorCode {
    type Error = ErrorCodeFromStrErr;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::CoreErrorFromString),
            2 => Ok(Self::UnexpectedCharacter),
            3 => Ok(Self::UnterminatedString),
            #[cfg(feature = "inline_comments")]
            4 => Ok(Self::UnterminatedComment),
            5 => Ok(Self::UnexpectedTokenKind),
            6 => Ok(Self::ExpectedTokenKind),
            7 => Ok(Self::InvalidAssignTarget),
            8 => Ok(Self::Over255Arguments),
            9 => Ok(Self::ExpectedName),
            10 => Ok(Self::ExpectedNumber),
            11 => Ok(Self::ReadInOwnInitilaizer),
            12 => Ok(Self::DefinedMultipleTimes),
            13 => Ok(Self::ReturnFromTop),
            14 => Ok(Self::ReturnValueFromInit),
            15 => Ok(Self::ThisOutsideClass),
            16 => Ok(Self::SelfReferentialInheritance),
            17 => Ok(Self::SuperOutsideSubclass),
            19 => Ok(Self::SuperClassIsNotAClass),
            20 => Ok(Self::StackOverflow),
            21 => Ok(Self::IncompatableUnaryOperator),
            22 => Ok(Self::IncompatableBinaryOperator),
            23 => Ok(Self::UndeclaredVariable),
            24 => Ok(Self::NonCallable),
            25 => Ok(Self::IncorrectArity),
            26 => Ok(Self::IncompatibleConversion),
            #[cfg(feature = "div_zero_error")]
            27 => Ok(Self::DivisionByZero),
            28 => Ok(Self::NonExistentField),
            29 => Ok(Self::CannotGetProperties),
            30 => Ok(Self::UninitializedVariable),
            31 => Ok(Self::IncorrectArgType),
            32 => Ok(Self::CouldNotConvert),
            34 => Ok(Self::OutOfBounds),
            35 => Ok(Self::NonIntegerIndex),
            36 => Ok(Self::NonPositiveIntegerOperand),
            37 => Ok(Self::IncorrectType),
            38 => Ok(Self::CannotIndexInto),
            39 => Ok(Self::IncorrectIndexKind),
            40 => Ok(Self::CannotSetProperties),
            41 => Ok(Self::PopEmptyList),
            254 => Ok(Self::AssertionFailed),
            255 => Ok(Self::UserForcedError),
            _ => Err(ErrorCodeFromStrErr::InvalidDiscriminant),
        }
    }
}

impl FromStr for ErrorCode {
    type Err = ErrorCodeFromStrErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();

        // Remove optional 'E' prefix
        let rest = s.strip_prefix('E').unwrap_or(s);

        // Ensure it's not empty and only digits
        if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
            return Err(ErrorCodeFromStrErr::InvalidFormat);
        }

        let code = rest.parse::<u8>()?;

        Self::try_from(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes_display() {
        assert_eq!(format!("{}", ErrorCode::CoreErrorFromString), "E00001");
        assert_eq!(format!("{}", ErrorCode::UnexpectedCharacter), "E00002");
        assert_eq!(format!("{}", ErrorCode::UnexpectedTokenKind), "E00005");
        assert_eq!(format!("{}", ErrorCode::ExpectedTokenKind), "E00006");
        assert_eq!(format!("{}", ErrorCode::InvalidAssignTarget), "E00007");
        assert_eq!(format!("{}", ErrorCode::Over255Arguments), "E00008");
        assert_eq!(format!("{}", ErrorCode::ExpectedName), "E00009");
        assert_eq!(format!("{}", ErrorCode::ExpectedNumber), "E00010");
        assert_eq!(format!("{}", ErrorCode::ReadInOwnInitilaizer), "E00011");
        assert_eq!(format!("{}", ErrorCode::DefinedMultipleTimes), "E00012");
    }
}
