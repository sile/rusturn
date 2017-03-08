use rustun::rfc5389::attributes::ErrorCode;

#[derive(Debug, Clone, Copy)]
pub struct Forbidden;
impl From<Forbidden> for ErrorCode {
    fn from(_: Forbidden) -> Self {
        ErrorCode::new(403, "Forbidden".to_string()).unwrap()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AllocationMismatch;
impl From<AllocationMismatch> for ErrorCode {
    fn from(_: AllocationMismatch) -> Self {
        ErrorCode::new(437, "Allocation Mismatch".to_string()).unwrap()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WrongCredentials;
impl From<WrongCredentials> for ErrorCode {
    fn from(_: WrongCredentials) -> Self {
        ErrorCode::new(441, "Wrong Credentials".to_string()).unwrap()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UnsupportedTransportProtocol;
impl From<UnsupportedTransportProtocol> for ErrorCode {
    fn from(_: UnsupportedTransportProtocol) -> Self {
        ErrorCode::new(442, "Unsupported Transport Protocol".to_string()).unwrap()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AllocationQuotaReached;
impl From<AllocationQuotaReached> for ErrorCode {
    fn from(_: AllocationQuotaReached) -> Self {
        ErrorCode::new(486, "Allocation Quota Reached".to_string()).unwrap()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InsufficientCapacity;
impl From<InsufficientCapacity> for ErrorCode {
    fn from(_: InsufficientCapacity) -> Self {
        ErrorCode::new(508, "InsufficientCapacity".to_string()).unwrap()
    }
}
