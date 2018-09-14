use stun_codec::rfc5389;

use Result;

#[derive(Debug)]
pub struct AuthParams {
    username: rfc5389::attributes::Username,
    password: String,
}
impl AuthParams {
    pub fn new(username: String, password: String) -> Result<Self> {
        let username = track!(rfc5389::attributes::Username::new(username))?;
        Ok(AuthParams { username, password })
    }
}
