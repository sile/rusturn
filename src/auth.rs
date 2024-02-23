use crate::attribute::Attribute;
use crate::{Error, ErrorKind, Result};
use stun_codec::rfc5389;
use stun_codec::Message;

#[derive(Debug, Clone)]
pub struct AuthParams {
    username: rfc5389::attributes::Username,
    password: String,
    realm: Option<rfc5389::attributes::Realm>,
    nonce: Option<rfc5389::attributes::Nonce>,
}
impl AuthParams {
    pub fn new(username: &str, password: &str) -> Result<Self> {
        let username = track!(rfc5389::attributes::Username::new(username.to_owned()))?;
        Ok(AuthParams {
            username,
            password: password.to_owned(),
            realm: None,
            nonce: None,
        })
    }

    pub fn with_realm_and_nonce(
        username: &str,
        password: &str,
        realm: &str,
        nonce: &str,
    ) -> Result<Self> {
        let username = track!(rfc5389::attributes::Username::new(username.to_owned()))?;
        let realm = track!(rfc5389::attributes::Realm::new(realm.to_owned()))?;
        let nonce = track!(rfc5389::attributes::Nonce::new(nonce.to_owned()))?;
        Ok(AuthParams {
            username,
            password: password.to_owned(),
            realm: Some(realm),
            nonce: Some(nonce),
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

    pub fn get_realm(&self) -> Option<&rfc5389::attributes::Realm> {
        self.realm.as_ref()
    }

    pub fn get_nonce(&self) -> Option<&rfc5389::attributes::Nonce> {
        self.nonce.as_ref()
    }

    pub fn add_auth_attributes<T>(&self, mut message: T) -> Result<()>
    where
        T: AsMut<Message<Attribute>>,
    {
        let realm = track_assert_some!(self.realm.clone(), ErrorKind::Other);
        let nonce = track_assert_some!(self.nonce.clone(), ErrorKind::Other);
        message.as_mut().add_attribute(self.username.clone());
        message.as_mut().add_attribute(realm.clone());
        message.as_mut().add_attribute(nonce);
        let mi = track!(
            rfc5389::attributes::MessageIntegrity::new_long_term_credential(
                message.as_mut(),
                &self.username,
                &realm,
                &self.password,
            )
        )?;
        message.as_mut().add_attribute(mi);
        Ok(())
    }

    pub fn validate(&self, mi: &rfc5389::attributes::MessageIntegrity) -> Result<()> {
        let realm = track_assert_some!(self.realm.as_ref(), ErrorKind::Other);
        track!(mi
            .check_long_term_credential(&self.username, realm, &self.password)
            .map_err(Error::from))?;
        Ok(())
    }
}
