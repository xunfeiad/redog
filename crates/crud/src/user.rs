use std::collections::BTreeMap;
use std::ops::Add;
use std::time::{SystemTime, UNIX_EPOCH};
use sqlx_postgres::PgPool;
use error::{RtcError, RtcResult};
use model::auth::user::User;
use crate::Crud;
use hmac::{Hmac, Mac};
use jwt::{Header, SignWithKey, Token, VerifyWithKey};
use sha2::{Digest, Sha384, Sha512};
use anyhow::{Context};
use base64::encode;
use config::constant::{SECRET_KEY, TIME_DELTA};
use async_trait::async_trait;
use sqlx::Type;


// pub struct Auth{
//     username: String,
//     password: String,
// }
//
// pub struct AuthReturn{
//     access_token: String
// }
//
// #[allow(async_fn_in_trait)]
#[async_trait]
pub trait Authentication{
    async fn validate_jwt(&self, pool: &PgPool, token: &str) -> RtcResult<()>;

    async fn login(&self, pool: &PgPool, user: &Auth) -> RtcResult<String>;
}
//

#[async_trait]
impl Authentication for User{
    async fn validate_jwt(&self, pool: &PgPool, token: &str) -> RtcResult<()> {
        let id = validate_jwt(token)?;
        self.get(pool, id as i64).await?;
        Ok(())
    }
    async fn login(
        &self,
        db: &PgPool,
        login_user: &Auth
    ) -> RtcResult<String> {
        let user = self.get_by_name(db, &login_user.username).await?;
        if sha256_hash(&login_user.password).ne(&user.password) {
            Err(RtcError::CustomerError("Incorrect username or password."))
        } else {
            let token = jwt_encrypt(user.id as usize)?;
            Ok(token)
        }
    }
}

//
// pub fn validate_jwt(token: &str) -> RtcResult<usize> {
//     let key: Hmac<Sha384> = Hmac::new_from_slice(SECRET_KEY.as_bytes())?;
//     let token: Token<Header, BTreeMap<String, String>, _> = token.verify_with_key(&key)?;
//     let claims = token.claims();
//     let time: u64 = claims
//         .get("sub")
//         .context("get jwt inner sub failed.")?
//         .parse()?;
//
//     let current_time = SystemTime::from(UNIX_EPOCH).elapsed()?.as_secs();
//     if time.lt(&current_time) {
//         return Err(RtcError::CustomerError("expired signature."));
//     }
//     let id: usize = claims
//         .get("iat")
//         .context("get jwt inner sub failed.")?
//         .parse()?;
//     Ok(id)
// }
//
// pub fn sha256_hash(str: &str) -> String {
//     let mut hasher = Sha512::new();
//     hasher.update(str);
//     hasher.update(SECRET_KEY);
//     let encrypted_password = hasher.finalize();
//     encode(encrypted_password)
// }
//
// pub fn jwt_encrypt(id: usize) -> RtcResult<String> {
//     let key: Hmac<Sha384> = Hmac::new_from_slice(SECRET_KEY.as_bytes())?;
//     let mut claims = BTreeMap::new();
//     let time = SystemTime::from(UNIX_EPOCH)
//         .elapsed()
//         .expect("Fetch system time failed.");
//     let sub_time = time.as_secs().add(TIME_DELTA);
//     claims.insert("sub", sub_time.to_string());
//     claims.insert("iat", id.to_string());
//     Ok(claims.sign_with_key(&key)?)
// }

