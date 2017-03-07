use rustun::Method as StunMethod;
use rustun::types::U12;

use methods;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Method {
    Allocate,
    Refresh,
    Send,
    Data,
    CreatePermission,
    ChannelBind,
}
impl StunMethod for Method {
    fn from_u12(value: U12) -> Option<Self> {
        match value.as_u16() {
            methods::METHOD_ALLOCATE => Some(Method::Allocate),
            methods::METHOD_REFRESH => Some(Method::Refresh),
            methods::METHOD_SEND => Some(Method::Send),
            methods::METHOD_DATA => Some(Method::Data),
            methods::METHOD_CREATE_PERMISSION => Some(Method::CreatePermission),
            methods::METHOD_CHANNEL_BIND => Some(Method::ChannelBind),
            _ => None,
        }
    }
    fn as_u12(&self) -> U12 {
        match *self {
            Method::Allocate => methods::Allocate.as_u12(),
            Method::Refresh => methods::Refresh.as_u12(),
            Method::Send => methods::Send.as_u12(),
            Method::Data => methods::Data.as_u12(),
            Method::CreatePermission => methods::CreatePermission.as_u12(),
            Method::ChannelBind => methods::ChannelBind.as_u12(),
        }
    }
}
