use rustun::method;
use rustun::types::U12;
use rustun::Method;

pub const METHOD_ALLOCATE: u16 = 0x003;
pub const METHOD_REFRESH: u16 = 0x004;
pub const METHOD_SEND: u16 = 0x006;
pub const METHOD_DATA: u16 = 0x007;
pub const METHOD_CREATE_PERMISSION: u16 = 0x008;
pub const METHOD_CHANNEL_BIND: u16 = 0x009;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Allocate;
impl Method for Allocate {
    fn from_u12(value: U12) -> Option<Self> {
        if value.as_u16() == METHOD_ALLOCATE {
            Some(Allocate)
        } else {
            None
        }
    }
    fn as_u12(&self) -> U12 {
        U12::from_u8(METHOD_ALLOCATE as u8)
    }
}
impl method::Requestable for Allocate {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Refresh;
impl Method for Refresh {
    fn from_u12(value: U12) -> Option<Self> {
        if value.as_u16() == METHOD_REFRESH {
            Some(Refresh)
        } else {
            None
        }
    }
    fn as_u12(&self) -> U12 {
        U12::from_u8(METHOD_REFRESH as u8)
    }
}
impl method::Requestable for Refresh {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Send;
impl Method for Send {
    fn from_u12(value: U12) -> Option<Self> {
        if value.as_u16() == METHOD_SEND {
            Some(Send)
        } else {
            None
        }
    }
    fn as_u12(&self) -> U12 {
        U12::from_u8(METHOD_SEND as u8)
    }
}
impl method::Indicatable for Send {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Data;
impl Method for Data {
    fn from_u12(value: U12) -> Option<Self> {
        if value.as_u16() == METHOD_DATA {
            Some(Data)
        } else {
            None
        }
    }
    fn as_u12(&self) -> U12 {
        U12::from_u8(METHOD_DATA as u8)
    }
}
impl method::Indicatable for Data {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CreatePermission;
impl Method for CreatePermission {
    fn from_u12(value: U12) -> Option<Self> {
        if value.as_u16() == METHOD_CREATE_PERMISSION {
            Some(CreatePermission)
        } else {
            None
        }
    }
    fn as_u12(&self) -> U12 {
        U12::from_u8(METHOD_CREATE_PERMISSION as u8)
    }
}
impl method::Requestable for CreatePermission {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChannelBind;
impl Method for ChannelBind {
    fn from_u12(value: U12) -> Option<Self> {
        if value.as_u16() == METHOD_CHANNEL_BIND {
            Some(ChannelBind)
        } else {
            None
        }
    }
    fn as_u12(&self) -> U12 {
        U12::from_u8(METHOD_CHANNEL_BIND as u8)
    }
}
impl method::Requestable for ChannelBind {}
