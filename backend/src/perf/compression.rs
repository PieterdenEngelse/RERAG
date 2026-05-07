//! LZ4 Compression for Vector Storage
//!
//! Provides fast compression/decompression for vector data.
//! LZ4 is optimized for speed over compression ratio, making it
//! ideal for frequently accessed vector data.
//!
//! # Performance
//! - Compression: ~500 MB/s
//! - Decompression: ~2 GB/s
//! - Ratio: ~2x for typical embedding vectors

use std::io::{Read, Write};
use tracing::debug;

/// Compress data using LZ4
///
/// Returns compressed bytes with size prepended for decompression.
pub fn compress(data: &[u8]) -> Vec<u8> {
    lz4_flex::compress_prepend_size(data)
}

/// Decompress LZ4 data
///
/// Expects size-prepended format from `compress()`.
pub fn decompress(compressed: &[u8]) -> Result<Vec<u8>, CompressionError> {
    lz4_flex::decompress_size_prepended(compressed)
        .map_err(|e| CompressionError::DecompressionFailed(e.to_string()))
}

/// Compress vector storage (rkyv bytes) with LZ4
pub fn compress_vectors(rkyv_bytes: &[u8]) -> Vec<u8> {
    let compressed = compress(rkyv_bytes);
    debug!(
        original_size = rkyv_bytes.len(),
        compressed_size = compressed.len(),
        ratio = format!("{:.2}x", rkyv_bytes.len() as f64 / compressed.len() as f64),
        "Compressed vectors"
    );
    compressed
}

/// Decompress vector storage
pub fn decompress_vectors(compressed: &[u8]) -> Result<Vec<u8>, CompressionError> {
    let decompressed = decompress(compressed)?;
    debug!(
        compressed_size = compressed.len(),
        decompressed_size = decompressed.len(),
        "Decompressed vectors"
    );
    Ok(decompressed)
}

/// Save compressed data to file
pub fn save_compressed<P: AsRef<std::path::Path>>(
    data: &[u8],
    path: P,
) -> Result<usize, CompressionError> {
    let compressed = compress(data);
    std::fs::write(path.as_ref(), &compressed)
        .map_err(|e| CompressionError::IoError(e.to_string()))?;
    Ok(compressed.len())
}

/// Load and decompress data from file
pub fn load_compressed<P: AsRef<std::path::Path>>(path: P) -> Result<Vec<u8>, CompressionError> {
    let compressed =
        std::fs::read(path.as_ref()).map_err(|e| CompressionError::IoError(e.to_string()))?;
    decompress(&compressed)
}

/// Streaming compression writer
pub struct CompressedWriter<W: Write> {
    inner: W,
    buffer: Vec<u8>,
    chunk_size: usize,
}

impl<W: Write> CompressedWriter<W> {
    pub fn new(writer: W) -> Self {
        Self::with_chunk_size(writer, 64 * 1024) // 64KB chunks
    }

    pub fn with_chunk_size(writer: W, chunk_size: usize) -> Self {
        Self {
            inner: writer,
            buffer: Vec::with_capacity(chunk_size),
            chunk_size,
        }
    }

    pub fn write_chunk(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.buffer.extend_from_slice(data);

        while self.buffer.len() >= self.chunk_size {
            let chunk: Vec<u8> = self.buffer.drain(..self.chunk_size).collect();
            let compressed = compress(&chunk);

            // Write chunk size then compressed data
            self.inner
                .write_all(&(compressed.len() as u32).to_le_bytes())?;
            self.inner.write_all(&compressed)?;
        }

        Ok(())
    }

    pub fn finish(mut self) -> std::io::Result<W> {
        if !self.buffer.is_empty() {
            let compressed = compress(&self.buffer);
            self.inner
                .write_all(&(compressed.len() as u32).to_le_bytes())?;
            self.inner.write_all(&compressed)?;
        }
        // Write end marker
        self.inner.write_all(&0u32.to_le_bytes())?;
        Ok(self.inner)
    }
}

/// Streaming decompression reader
pub struct CompressedReader<R: Read> {
    inner: R,
    buffer: Vec<u8>,
    position: usize,
}

impl<R: Read> CompressedReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            inner: reader,
            buffer: Vec::new(),
            position: 0,
        }
    }

    fn read_next_chunk(&mut self) -> std::io::Result<bool> {
        let mut size_buf = [0u8; 4];
        if self.inner.read_exact(&mut size_buf).is_err() {
            return Ok(false);
        }

        let chunk_size = u32::from_le_bytes(size_buf) as usize;
        if chunk_size == 0 {
            return Ok(false); // End marker
        }

        let mut compressed = vec![0u8; chunk_size];
        self.inner.read_exact(&mut compressed)?;

        self.buffer = decompress(&compressed)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        self.position = 0;

        Ok(true)
    }
}

impl<R: Read> Read for CompressedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.position >= self.buffer.len() && !self.read_next_chunk()? {
            return Ok(0);
        }

        let available = self.buffer.len() - self.position;
        let to_read = buf.len().min(available);
        buf[..to_read].copy_from_slice(&self.buffer[self.position..self.position + to_read]);
        self.position += to_read;

        Ok(to_read)
    }
}

#[derive(Debug)]
pub enum CompressionError {
    DecompressionFailed(String),
    IoError(String),
}

impl std::fmt::Display for CompressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DecompressionFailed(msg) => write!(f, "Decompression failed: {}", msg),
            Self::IoError(msg) => write!(f, "IO error: {}", msg),
        }
    }
}

impl std::error::Error for CompressionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_decompress() {
        let data = b"Hello, World! This is a test of LZ4 compression.";
        let compressed = compress(data);
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(data.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_vector_compression() {
        // Simulate vector data (floats as bytes)
        let vectors: Vec<f32> = (0..1000).map(|i| i as f32 * 0.001).collect();
        let bytes: Vec<u8> = vectors.iter().flat_map(|f| f.to_le_bytes()).collect();

        let compressed = compress_vectors(&bytes);
        let decompressed = decompress_vectors(&compressed).unwrap();

        assert_eq!(bytes, decompressed);
        println!(
            "Original: {} bytes, Compressed: {} bytes, Ratio: {:.2}x",
            bytes.len(),
            compressed.len(),
            bytes.len() as f64 / compressed.len() as f64
        );
    }
}
