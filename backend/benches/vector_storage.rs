//! Benchmarks for vector storage operations
//!
//! Run with: cargo bench --bench vector_storage
//!
//! This benchmark suite compares:
//! - JSON vs rkyv serialization/deserialization
//! - Memory-mapped vs regular file loading
//! - Different vector counts and dimensions

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::Rng;
use std::collections::HashMap;
use std::time::Duration;

/// Generate random test vectors
fn generate_vectors(count: usize, dim: usize) -> Vec<Vec<f32>> {
    let mut rng = rand::thread_rng();
    (0..count)
        .map(|_| (0..dim).map(|_| rng.gen::<f32>()).collect())
        .collect()
}

/// Generate random doc_id to index mapping
fn generate_doc_map(count: usize) -> HashMap<String, usize> {
    (0..count).map(|i| (format!("doc_{:08}", i), i)).collect()
}

// ============================================================================
// JSON Serialization Benchmarks
// ============================================================================

fn bench_json_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_serialize");
    group.measurement_time(Duration::from_secs(10));

    for count in [100, 1000, 5000].iter() {
        let vectors = generate_vectors(*count, 384);
        let doc_map = generate_doc_map(*count);

        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(
            BenchmarkId::new("vectors", count),
            &(&vectors, &doc_map),
            |b, (vecs, map)| {
                b.iter(|| {
                    let storage = serde_json::json!({
                        "vectors": vecs,
                        "doc_id_to_vector_idx": map,
                    });
                    black_box(serde_json::to_string(&storage).unwrap())
                });
            },
        );
    }
    group.finish();
}

fn bench_json_deserialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_deserialize");
    group.measurement_time(Duration::from_secs(10));

    for count in [100, 1000, 5000].iter() {
        let vectors = generate_vectors(*count, 384);
        let doc_map = generate_doc_map(*count);
        let storage = serde_json::json!({
            "vectors": vectors,
            "doc_id_to_vector_idx": doc_map,
        });
        let json = serde_json::to_string(&storage).unwrap();

        group.throughput(Throughput::Bytes(json.len() as u64));
        group.bench_with_input(BenchmarkId::new("vectors", count), &json, |b, json| {
            b.iter(|| {
                let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
                black_box(parsed)
            });
        });
    }
    group.finish();
}

// ============================================================================
// rkyv Serialization Benchmarks
// ============================================================================

/// rkyv-compatible structure for benchmarking
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
struct BenchVectorStorage {
    version: u32,
    vectors: Vec<Vec<f32>>,
    doc_id_to_idx: Vec<(String, u32)>,
}

fn bench_rkyv_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("rkyv_serialize");
    group.measurement_time(Duration::from_secs(10));

    for count in [100, 1000, 5000].iter() {
        let vectors = generate_vectors(*count, 384);
        let doc_map: Vec<(String, u32)> = (0..*count)
            .map(|i| (format!("doc_{:08}", i), i as u32))
            .collect();

        let storage = BenchVectorStorage {
            version: 1,
            vectors: vectors.clone(),
            doc_id_to_idx: doc_map,
        };

        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(
            BenchmarkId::new("vectors", count),
            &storage,
            |b, storage| {
                b.iter(|| black_box(rkyv::to_bytes::<rkyv::rancor::Error>(storage).unwrap()));
            },
        );
    }
    group.finish();
}

fn bench_rkyv_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("rkyv_access");
    group.measurement_time(Duration::from_secs(10));

    for count in [100, 1000, 5000].iter() {
        let vectors = generate_vectors(*count, 384);
        let doc_map: Vec<(String, u32)> = (0..*count)
            .map(|i| (format!("doc_{:08}", i), i as u32))
            .collect();

        let storage = BenchVectorStorage {
            version: 1,
            vectors,
            doc_id_to_idx: doc_map,
        };
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&storage).unwrap();

        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::new("vectors", count), &bytes, |b, bytes| {
            b.iter(|| {
                let archived =
                    rkyv::access::<ArchivedBenchVectorStorage, rkyv::rancor::Error>(bytes).unwrap();
                black_box(archived.vectors.len())
            });
        });
    }
    group.finish();
}

fn bench_rkyv_deserialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("rkyv_deserialize");
    group.measurement_time(Duration::from_secs(10));

    for count in [100, 1000, 5000].iter() {
        let vectors = generate_vectors(*count, 384);
        let doc_map: Vec<(String, u32)> = (0..*count)
            .map(|i| (format!("doc_{:08}", i), i as u32))
            .collect();

        let storage = BenchVectorStorage {
            version: 1,
            vectors,
            doc_id_to_idx: doc_map,
        };
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&storage).unwrap();

        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::new("vectors", count), &bytes, |b, bytes| {
            b.iter(|| {
                let archived =
                    rkyv::access::<ArchivedBenchVectorStorage, rkyv::rancor::Error>(bytes).unwrap();
                // Full deserialization
                let vecs: Vec<Vec<f32>> = archived
                    .vectors
                    .iter()
                    .map(|v| v.iter().map(|f| f.to_native()).collect())
                    .collect();
                black_box(vecs)
            });
        });
    }
    group.finish();
}

// ============================================================================
// Size Comparison
// ============================================================================

fn bench_size_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("size_comparison");

    for count in [100, 1000, 5000].iter() {
        let vectors = generate_vectors(*count, 384);
        let doc_map = generate_doc_map(*count);

        // JSON size
        let json_storage = serde_json::json!({
            "vectors": vectors,
            "doc_id_to_vector_idx": doc_map,
        });
        let json_bytes = serde_json::to_string(&json_storage).unwrap();

        // rkyv size
        let rkyv_storage = BenchVectorStorage {
            version: 1,
            vectors: vectors.clone(),
            doc_id_to_idx: doc_map
                .iter()
                .map(|(k, v)| (k.clone(), *v as u32))
                .collect(),
        };
        let rkyv_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&rkyv_storage).unwrap();

        println!("\n=== {} vectors ===", count);
        println!(
            "JSON size: {} bytes ({:.2} KB)",
            json_bytes.len(),
            json_bytes.len() as f64 / 1024.0
        );
        println!(
            "rkyv size: {} bytes ({:.2} KB)",
            rkyv_bytes.len(),
            rkyv_bytes.len() as f64 / 1024.0
        );
        println!(
            "Ratio: {:.2}x smaller",
            json_bytes.len() as f64 / rkyv_bytes.len() as f64
        );

        // Benchmark just to have something to measure
        group.bench_function(BenchmarkId::new("json_size", count), |b| {
            b.iter(|| black_box(json_bytes.len()))
        });
        group.bench_function(BenchmarkId::new("rkyv_size", count), |b| {
            b.iter(|| black_box(rkyv_bytes.len()))
        });
    }
    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    benches,
    bench_json_serialize,
    bench_json_deserialize,
    bench_rkyv_serialize,
    bench_rkyv_access,
    bench_rkyv_deserialize,
    bench_size_comparison,
);

criterion_main!(benches);
