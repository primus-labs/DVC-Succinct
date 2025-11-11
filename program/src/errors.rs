#[repr(i16)]
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum ZkErrorCode {
    ParseAttestationData = 1001,
    GetAttestorAddressFail,
    VerifyAttestation,
    InvalidRequestLength,
    InvalidMessagesLength,
    GetJsonValueFail,
    InvalidJsonValueSize,
    CannotFoundTimestamp,
    ParseTimestampFailed,
    InvalidRequestOrder,
    InvalidRequestUrl,
    DuplicateAccount,
    NotSupportSource,
    UpTimeNotEnough,
    EmptyPlainResponse,
}

#[derive(Debug)]
pub struct ZktlsError {
    code: ZkErrorCode,
    msg: String,
}

impl ZktlsError {
    pub fn new(code: ZkErrorCode, msg: impl Into<String>) -> Self {
        Self { code, msg: msg.into() }
    }
    pub fn icode(&self) -> i16 {
        self.code.clone() as i16
    }
    #[allow(dead_code)]
    pub fn msg(&self) -> String {
        self.msg.clone()
    }
}

impl std::fmt::Display for ZktlsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZktlsError(code: {}, msg: {})", self.icode(), self.msg)
    }
}

impl std::error::Error for ZktlsError {}

#[macro_export]
macro_rules! ensure_zk {
    ($cond:expr, $err:expr) => {
        if !$cond {
            return Err($err.into());
        }
    };
}

#[macro_export]
macro_rules! zkerr {
    ($code:expr, $msg:expr) => {
        ZktlsError::new($code, $msg)
    };
    ($code:expr) => {
        zkerr!($code, stringify!($code).to_string())
    };
}
