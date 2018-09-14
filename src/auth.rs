use rustun::message::Request;
use stun_codec::rfc5389;

use attribute::Attribute;
use {Error, ErrorKind, Result};

#[derive(Debug, Clone)]
pub struct AuthParams {
    username: rfc5389::attributes::Username,
    password: String,
    realm: Option<rfc5389::attributes::Realm>,
    nonce: Option<rfc5389::attributes::Nonce>,
}
impl AuthParams {
    pub fn new(username: String, password: String) -> Result<Self> {
        let username = track!(rfc5389::attributes::Username::new(username))?;
        Ok(AuthParams {
            username,
            password,
            realm: None,
            nonce: None,
        })
    }

    pub fn has_realm(&self) -> bool {
        self.realm.is_some()
    }

    pub fn has_nonce(&self) -> bool {
        self.nonce.is_some()
    }

    pub fn set_realm(&mut self, realm: rfc5389::attributes::Realm) {
        self.realm = Some(realm);
    }

    pub fn set_nonce(&mut self, nonce: rfc5389::attributes::Nonce) {
        self.nonce = Some(nonce);
    }

    pub fn add_auth_attributes(&self, request: &mut Request<Attribute>) -> Result<()> {
        let realm = track_assert_some!(self.realm.clone(), ErrorKind::Other);
        let nonce = track_assert_some!(self.nonce.clone(), ErrorKind::Other);
        request.add_attribute(self.username.clone().into());
        request.add_attribute(realm.clone().into());
        request.add_attribute(nonce.into());
        let mi = track!(
            rfc5389::attributes::MessageIntegrity::new_long_term_credential(
                request.as_ref(),
                &self.username,
                &realm,
                &self.password,
            )
        )?;
        request.add_attribute(mi.into());
        Ok(())
    }

    pub fn validate(&self, mi: &rfc5389::attributes::MessageIntegrity) -> Result<()> {
        let realm = track_assert_some!(self.realm.as_ref(), ErrorKind::Other);
        track!(
            mi.check_long_term_credential(&self.username, realm, &self.password)
                .map_err(Error::from)
        )?;
        Ok(())
    }
}
