use crate::Args;

#[derive(Clone, Debug)]
pub struct Argon {
    secret: String,
    memory_size: Option<u32>,
    iterations: Option<u32>,
}

impl Argon {
    pub fn new(args: &Args) -> Self {
        let Args {
            argon_secret,
            argon_memory_size,
            argon_iterations,
            ..
        } = args;
        Self {
            secret: argon_secret.to_owned(),
            memory_size: argon_memory_size.to_owned(),
            iterations: argon_iterations.to_owned(),
        }
    }

    pub fn hasher(&self) -> argonautica::Hasher<'static> {
        let mut hasher = argonautica::Hasher::default();
        let mut hasher = hasher.with_secret_key(&self.secret);
        if let Some(memory_size) = self.memory_size {
            hasher = hasher.configure_memory_size(memory_size);
        }
        if let Some(iterations) = self.iterations {
            hasher = hasher.configure_iterations(iterations);
        }
        hasher.to_owned()
    }

    pub fn verifier(&self) -> argonautica::Verifier<'static> {
        let mut verifier = argonautica::Verifier::default();
        let verifier = verifier.with_secret_key(&self.secret);
        verifier.to_owned()
    }
}
