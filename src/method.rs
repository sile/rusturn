use rustun::Method as StunMethod;
use rustun::types::U12;
use rustun::rfc5389;

use rfc5766;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Method {
    Binding,
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
            rfc5389::methods::METHOD_BINDING => Some(Method::Binding),
            rfc5766::methods::METHOD_ALLOCATE => Some(Method::Allocate),
            rfc5766::methods::METHOD_REFRESH => Some(Method::Refresh),
            rfc5766::methods::METHOD_SEND => Some(Method::Send),
            rfc5766::methods::METHOD_DATA => Some(Method::Data),
            rfc5766::methods::METHOD_CREATE_PERMISSION => Some(Method::CreatePermission),
            rfc5766::methods::METHOD_CHANNEL_BIND => Some(Method::ChannelBind),
            _ => None,
        }
    }
    fn as_u12(&self) -> U12 {
        match *self {
            Method::Binding => rfc5389::methods::Binding.as_u12(),
            Method::Allocate => rfc5766::methods::Allocate.as_u12(),
            Method::Refresh => rfc5766::methods::Refresh.as_u12(),
            Method::Send => rfc5766::methods::Send.as_u12(),
            Method::Data => rfc5766::methods::Data.as_u12(),
            Method::CreatePermission => rfc5766::methods::CreatePermission.as_u12(),
            Method::ChannelBind => rfc5766::methods::ChannelBind.as_u12(),
        }
    }
}
impl From<rfc5389::methods::Binding> for Method {
    fn from(_: rfc5389::methods::Binding) -> Self {
        Method::Binding
    }
}
impl From<rfc5766::methods::Allocate> for Method {
    fn from(_: rfc5766::methods::Allocate) -> Self {
        Method::Allocate
    }
}
impl From<rfc5766::methods::Refresh> for Method {
    fn from(_: rfc5766::methods::Refresh) -> Self {
        Method::Refresh
    }
}
impl From<rfc5766::methods::Send> for Method {
    fn from(_: rfc5766::methods::Send) -> Self {
        Method::Send
    }
}
impl From<rfc5766::methods::Data> for Method {
    fn from(_: rfc5766::methods::Data) -> Self {
        Method::Data
    }
}
impl From<rfc5766::methods::CreatePermission> for Method {
    fn from(_: rfc5766::methods::CreatePermission) -> Self {
        Method::CreatePermission
    }
}
impl From<rfc5766::methods::ChannelBind> for Method {
    fn from(_: rfc5766::methods::ChannelBind) -> Self {
        Method::ChannelBind
    }
}
