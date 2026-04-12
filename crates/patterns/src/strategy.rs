//! Trait objects and the Strategy pattern.
//!
//! Dynamic dispatch via `dyn Trait` lets callers swap implementations at runtime.

/// Strategy trait for data compression.
pub trait Compressor: Send + Sync {
    /// Compress the input bytes and return the compressed output.
    fn compress(&self, data: &[u8]) -> Vec<u8>;
    /// Return the name of this compression strategy.
    fn name(&self) -> &str;
}

/// Identity compressor — no-op passthrough.
pub struct NoopCompressor;

impl Compressor for NoopCompressor {
    fn compress(&self, data: &[u8]) -> Vec<u8> {
        data.to_vec()
    }
    fn name(&self) -> &str {
        "noop"
    }
}

/// Run-length encoding compressor (simple demonstration).
pub struct RleCompressor;

impl Compressor for RleCompressor {
    fn compress(&self, data: &[u8]) -> Vec<u8> {
        if data.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut count: u8 = 1;
        let mut prev = data[0];
        for &b in &data[1..] {
            if b == prev && count < 255 {
                count += 1;
            } else {
                out.push(count);
                out.push(prev);
                prev = b;
                count = 1;
            }
        }
        out.push(count);
        out.push(prev);
        out
    }
    fn name(&self) -> &str {
        "rle"
    }
}

/// A pipeline that uses a compressor strategy via trait object.
pub struct Pipeline {
    compressor: Box<dyn Compressor>,
}

impl Pipeline {
    /// Create a new pipeline with the given compressor strategy.
    pub fn new(compressor: Box<dyn Compressor>) -> Self {
        Self { compressor }
    }

    /// Compress `data` using the configured strategy.
    pub fn process(&self, data: &[u8]) -> Vec<u8> {
        self.compressor.compress(data)
    }

    /// Return the name of the active compressor.
    pub fn compressor_name(&self) -> &str {
        self.compressor.name()
    }
}

/// Enum dispatch alternative — avoids heap allocation, still polymorphic.
pub enum CompressorKind {
    /// No-op passthrough.
    Noop,
    /// Run-length encoding.
    Rle,
}

impl Compressor for CompressorKind {
    fn compress(&self, data: &[u8]) -> Vec<u8> {
        match self {
            Self::Noop => NoopCompressor.compress(data),
            Self::Rle => RleCompressor.compress(data),
        }
    }
    fn name(&self) -> &str {
        match self {
            Self::Noop => "noop",
            Self::Rle => "rle",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_passthrough() {
        let p = Pipeline::new(Box::new(NoopCompressor));
        assert_eq!(p.process(b"hello"), b"hello");
        assert_eq!(p.compressor_name(), "noop");
    }

    #[test]
    fn rle_compresses_runs() {
        let compressed = RleCompressor.compress(b"aaabbc");
        // 3,'a', 2,'b', 1,'c'
        assert_eq!(compressed, vec![3, b'a', 2, b'b', 1, b'c']);
    }

    #[test]
    fn rle_empty() {
        assert!(RleCompressor.compress(b"").is_empty());
    }

    #[test]
    fn enum_dispatch() {
        let c = CompressorKind::Rle;
        assert_eq!(c.name(), "rle");
        assert!(!c.compress(b"aaa").is_empty());
    }

    #[test]
    fn swap_strategy_at_runtime() {
        let strategies: Vec<Box<dyn Compressor>> =
            vec![Box::new(NoopCompressor), Box::new(RleCompressor)];
        let names: Vec<&str> = strategies.iter().map(|s| s.name()).collect();
        assert_eq!(names, vec!["noop", "rle"]);
    }
}
