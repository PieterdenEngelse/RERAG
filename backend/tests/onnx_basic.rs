//! Basic ONNX Runtime test

#[test]
fn test_onnx_environment() {
    use ort::Environment;

    let result = Environment::builder().with_name("test").build();

    match result {
        Ok(_) => println!("✅ ONNX Runtime loaded successfully"),
        Err(e) => panic!("❌ Failed to load ONNX Runtime: {}", e),
    }
}
