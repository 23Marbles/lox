use std::{error::Error, fmt::Display, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum ErrorCode {
    CoreErrorFromString = 1,
    UnexpectedCharacter = 2,
    UnterminatedString = 3,
    #[cfg(feature = "inline_comments")]
    UnterminatedComment = 4,
    UnexpectedTokenKind = 5,
    ExpectedTokenKind = 6,
    InvalidAssignTarget = 7,
    TooManyArguments = 8,
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
    DivisionByZero = 27,
    UndefinedProperty = 28,
    NotInstance = 29,
}

impl Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "E{:05}", *self as u8)
    }
}

#[derive(Debug, Clone)]
pub enum ErrorCodeFromStrErr {
    U8ParseError(std::num::ParseIntError),
    InvalidDiscriminant,
    InvalidFormat,
}

impl From<std::num::ParseIntError> for ErrorCodeFromStrErr {
    fn from(err: std::num::ParseIntError) -> Self {
        ErrorCodeFromStrErr::U8ParseError(err)
    }
}

impl Display for ErrorCodeFromStrErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
            8 => Ok(Self::TooManyArguments),
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
            27 => Ok(Self::DivisionByZero),
            28 => Ok(Self::UndefinedProperty),
            29 => Ok(Self::NotInstance),
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
        assert_eq!(format!("{}", ErrorCode::TooManyArguments), "E00008");
        assert_eq!(format!("{}", ErrorCode::ExpectedName), "E00009");
        assert_eq!(format!("{}", ErrorCode::ExpectedNumber), "E00010");
        assert_eq!(format!("{}", ErrorCode::ReadInOwnInitilaizer), "E00011");
        assert_eq!(format!("{}", ErrorCode::DefinedMultipleTimes), "E00012");
    }
}
