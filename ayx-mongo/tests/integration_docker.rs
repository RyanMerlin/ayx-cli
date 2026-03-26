use std::fs;
use std::process::Command;

#[test]
#[ignore]
fn managed_mongo_docker_smoke() {
    if std::env::var("AYX_RUN_DOCKER_TESTS").ok().as_deref() != Some("1") {
        eprintln!("skipping: set AYX_RUN_DOCKER_TESTS=1");
        return;
    }

    let image = "mongo:7";
    let cname = "ayx-mongo-it";

    let _ = Command::new("docker").args(["rm", "-f", cname]).output();

    let run = Command::new("docker")
        .args(["run", "-d", "--name", cname, "-p", "27027:27017", image])
        .output()
        .expect("docker run failed");
    assert!(
        run.status.success(),
        "docker run stderr={}",
        String::from_utf8_lossy(&run.stderr)
    );

    let cfg_path = std::env::temp_dir().join("ayx-mongo-it-config.yaml");
    fs::write(
        &cfg_path,
        r#"profile_name: it
mongo:
  mode: managed
  databases:
    gallery_name: AlteryxGallery
    service_name: AlteryxService
  embedded:
    runtime_settings_path: null
    alteryx_service_path: null
    restore_target_path: null
  managed:
    url: null
    host: localhost
    port: 27027
    auth_database: admin
    username: null
    password: null
    tls:
      enabled: false
      ca_path: null
      cert_path: null
      key_path: null
      allow_invalid_hostnames: false
    timeout_ms: 5000
    retry_count: 1
    max_pool_size: 5
"#,
    )
    .expect("failed writing cfg");

    let backup_dir = std::env::temp_dir().join("ayx-mongo-it-backup");
    let audits_dir = std::env::temp_dir().join("ayx-mongo-it-audits");

    let status = Command::new("cargo")
        .args([
            "run",
            "-p",
            "ayx",
            "--",
            "--output",
            "json",
            "mongo",
            "backup",
            "--profile",
            cfg_path.to_string_lossy().as_ref(),
            "--output-dir",
            backup_dir.to_string_lossy().as_ref(),
            "--apply",
            "--audit-dir",
            audits_dir.to_string_lossy().as_ref(),
        ])
        .status()
        .expect("failed to run backup");

    let _ = Command::new("docker").args(["rm", "-f", cname]).status();

    assert!(status.success(), "backup command should succeed");
}
