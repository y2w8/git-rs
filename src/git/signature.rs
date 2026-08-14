use gix::actor::SignatureRef;

use crate::time::Time;

#[derive(Clone)]
pub struct GitSignature {
    pub name: String,
    pub email: String,
    pub time: Time,
}

impl GitSignature {
    pub fn from_gix(sig: SignatureRef) -> Self {
        GitSignature {
            name: sig.name.to_string(),
            email: sig.email.to_string(),
            time: Time::new(sig.time().unwrap()),
        }
    }

    pub fn avatar_url(&self, size: u32) -> String {
        let normalized = self.email.trim().to_lowercase();
        let hash = md5::compute(normalized.as_bytes());
        format!(
            "https://www.gravatar.com/avatar/{:x}?s={}&d=retro",
            hash, size
        )
    }
}
