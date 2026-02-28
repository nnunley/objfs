// Integration tests for HTTP-based Remote CAS client
// These tests require a running NativeLink instance

use objfs::re_client::{Digest, RemoteCas, HttpRemoteCas};

#[test]
#[ignore] // Run with: cargo test --ignored http_cas_upload_and_download
fn test_http_cas_upload_and_download() {
    // Connect to local NativeLink instance
    let cas = HttpRemoteCas::new("http://scheduler-host:50051".to_string());

    let data = b"test data from objfs";

    // Upload
    let digest = cas.upload(data).unwrap();

    // Should exist
    assert!(cas.exists(&digest).unwrap());

    // Download
    let retrieved = cas.download(&digest).unwrap();
    assert_eq!(retrieved, data);
}

#[test]
#[ignore]
fn test_http_cas_nonexistent_blob() {
    let cas = HttpRemoteCas::new("http://scheduler-host:50051".to_string());

    // Fake digest
    let fake_digest = Digest::new("0".repeat(64), 0);

    // Should not exist
    assert!(!cas.exists(&fake_digest).unwrap());

    // Download should fail
    let result = cas.download(&fake_digest);
    assert!(result.is_err());
}

#[test]
#[ignore]
fn test_http_cas_large_blob() {
    let cas = HttpRemoteCas::new("http://scheduler-host:50051".to_string());

    // Create a 1MB blob
    let data = vec![42u8; 1024 * 1024];

    let digest = cas.upload(&data).unwrap();
    assert_eq!(digest.size_bytes, data.len() as i64);

    let retrieved = cas.download(&digest).unwrap();
    assert_eq!(retrieved, data);
}
