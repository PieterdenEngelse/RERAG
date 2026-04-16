//! Basic ONNX Runtime test

#[test]
fn test_onnx_environment() {
    let ok = ort::init().with_name("test").commit();
    assert!(ok, "Failed to initialize ONNX Runtime environment");
    println!("ONNX Runtime loaded successfully");
}
