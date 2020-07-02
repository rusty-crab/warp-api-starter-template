use crate::auth;

type DateTimeUtc = chrono::DateTime<chrono::Utc>;
#[derive(Clone, Debug)]
pub struct Jwt {
    secret: String,
}

impl Jwt {
    pub fn new(secret: &str) -> Self {
        Self {
            secret: secret.to_owned(),
        }
    }

    pub fn encode(&self, claims: auth::Claims, _expiry: DateTimeUtc) -> anyhow::Result<String> {
        let registered = biscuit::RegisteredClaims::default();
        let private = claims;
        let claims = biscuit::ClaimsSet::<auth::Claims> {
            registered,
            private,
        };

        let jwt = biscuit::JWT::new_decoded(
            From::from(biscuit::jws::RegisteredHeader {
                algorithm: biscuit::jwa::SignatureAlgorithm::HS256,
                ..Default::default()
            }),
            claims,
        );

        let secret = biscuit::jws::Secret::bytes_from_str(&self.secret);

        jwt.into_encoded(&secret)
            .map(|t| t.unwrap_encoded().to_string())
            .map_err(|e| e.into())
    }

    pub fn decode(&self, token: &str) -> anyhow::Result<auth::Claims> {
        let token = biscuit::JWT::<auth::Claims, biscuit::Empty>::new_encoded(&token);
        let secret = biscuit::jws::Secret::bytes_from_str(&self.secret);
        let token = token.into_decoded(&secret, biscuit::jwa::SignatureAlgorithm::HS256)?;
        let payload = token.payload()?.private.to_owned();
        Ok(payload)
    }
}
