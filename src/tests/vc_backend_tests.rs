use crate::kernel::vc::backends::ShellTemplateBackend;
use crate::kernel::vc::VcBackend;

#[test]
fn shell_template_detect_with_marker() {
    let tmp = std::env::temp_dir().join("magma_vc_shell_detect");
    let sub = tmp.join("subdir").join("deep");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(tmp.join(".magma_test_marker"), "").unwrap();

    let backend = ShellTemplateBackend {
        name: "test-vcs".to_string(),
        detect_marker: ".magma_test_marker".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };

    assert!(backend.detect(&sub), "should detect from deep subdirectory");
    assert!(backend.detect(&tmp), "should detect from root");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn shell_template_detect_without_marker() {
    let tmp = std::env::temp_dir().join("magma_vc_no_marker");
    std::fs::create_dir_all(&tmp).unwrap();

    let backend = ShellTemplateBackend {
        name: "test-vcs".to_string(),
        detect_marker: ".nonexistent_marker_xyz".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };

    assert!(!backend.detect(&tmp));

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn shell_template_name() {
    let backend = ShellTemplateBackend {
        name: "fossil".to_string(),
        detect_marker: ".fslckout".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    assert_eq!(backend.name(), "fossil");
}

#[test]
fn shell_template_status_errors_when_no_cmd() {
    let backend = ShellTemplateBackend {
        name: "fossil".to_string(),
        detect_marker: ".fslckout".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let tmp = std::env::temp_dir().join("magma_vc_no_status_cmd");
    std::fs::create_dir_all(&tmp).unwrap();
    let r = backend.status(&tmp);
    assert!(r.is_err());
    let err = r.unwrap_err();
    assert!(err.contains("no status-cmd"), "error should mention missing cmd: {err}");
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn shell_template_diff_errors_when_no_cmd() {
    let backend = ShellTemplateBackend {
        name: "fossil".to_string(),
        detect_marker: ".fslckout".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let tmp = std::env::temp_dir();
    let r = backend.diff(&tmp, Some("file.txt"));
    assert!(r.is_err());
    assert!(r.unwrap_err().contains("no diff-cmd"));
}

#[test]
fn shell_template_root_returns_none_when_no_cmd() {
    let backend = ShellTemplateBackend {
        name: "test".to_string(),
        detect_marker: ".marker".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let tmp = std::env::temp_dir();
    assert!(backend.root(&tmp).is_none());
}

#[test]
fn shell_template_push_errors_when_no_cmd() {
    let backend = ShellTemplateBackend {
        name: "test".to_string(),
        detect_marker: ".marker".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let tmp = std::env::temp_dir();
    let r = backend.push(&tmp);
    assert!(r.is_err());
    assert!(r.unwrap_err().contains("no push-cmd"));
}

#[test]
fn shell_template_stage_errors_when_no_cmd() {
    let backend = ShellTemplateBackend {
        name: "test".to_string(),
        detect_marker: ".marker".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let tmp = std::env::temp_dir();
    let r = backend.stage(&tmp, "f");
    assert!(r.is_err());
    assert!(r.unwrap_err().contains("staging not supported"));
}

#[test]
fn shell_template_unstage_errors_when_no_cmd() {
    let backend = ShellTemplateBackend {
        name: "test".to_string(),
        detect_marker: ".marker".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let tmp = std::env::temp_dir();
    let r = backend.unstage(&tmp, "f");
    assert!(r.is_err());
    assert!(r.unwrap_err().contains("unstaging not supported"));
}

#[test]
fn shell_template_commit_errors_when_no_cmd() {
    let backend = ShellTemplateBackend {
        name: "test".to_string(),
        detect_marker: ".marker".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let tmp = std::env::temp_dir();
    let r = backend.commit(&tmp, "msg");
    assert!(r.is_err());
    assert!(r.unwrap_err().contains("no commit-cmd"));
}

#[test]
fn shell_template_log_errors_when_no_cmd() {
    let backend = ShellTemplateBackend {
        name: "test".to_string(),
        detect_marker: ".marker".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let tmp = std::env::temp_dir();
    let r = backend.log(&tmp, 10);
    assert!(r.is_err());
    assert!(r.unwrap_err().contains("no log-cmd"));
}

#[test]
fn shell_template_blame_errors_when_no_cmd() {
    let backend = ShellTemplateBackend {
        name: "test".to_string(),
        detect_marker: ".marker".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let tmp = std::env::temp_dir();
    let r = backend.blame(&tmp, "f");
    assert!(r.is_err());
    assert!(r.unwrap_err().contains("no blame-cmd"));
}
