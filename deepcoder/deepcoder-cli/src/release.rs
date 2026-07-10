//! Release artifact helpers.

use sha2::{Digest, Sha256};

pub fn artifact_name(version: &str, target_triple: &str) -> String {
    format!("deepcoder-{version}-{target_triple}.tar.gz")
}

pub fn checksum_line(bytes: &[u8], filename: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    format!("{digest:x}  {filename}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn release_artifact_name_format() {
        let name = artifact_name("0.1.0", "x86_64-pc-windows-msvc");
        assert!(name.contains("deepcoder"));
        assert!(name.contains("0.1.0"));
        assert!(name.contains("x86_64-pc-windows-msvc"));
    }

    #[test]
    fn checksum_file_format() {
        let line = checksum_line(b"abc", "deepcoder-0.1.0-linux.tar.gz");
        assert_eq!(
            line,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  deepcoder-0.1.0-linux.tar.gz"
        );
    }

    #[test]
    fn windows_web_launcher_files_exist() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("crate should live under repo root");
        for path in [
            "StartDeepCoderWeb.ps1",
            "启动DeepCoder网页版.ps1",
            "启动DeepCoder网页版.bat",
            "deepcoder-web/package.json",
            "deepcoder-web/package-lock.json",
            "deepcoder-web/src/App.jsx",
        ] {
            assert!(
                repo_root.join(path).exists(),
                "expected release launcher input to exist: {path}"
            );
        }
    }

    #[test]
    fn windows_release_workflow_packages_web_launcher() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("crate should live under repo root");
        let workflow = std::fs::read_to_string(repo_root.join(".github/workflows/release.yml"))
            .expect("release workflow should be readable");
        for needle in [
            "StartDeepCoderWeb.ps1",
            "启动DeepCoder网页版.bat",
            "启动DeepCoder网页版.ps1",
            "../deepcoder-web/src",
        ] {
            assert!(
                workflow.contains(needle),
                "release workflow should package {needle}"
            );
        }
    }

    #[test]
    fn web_ci_runs_tests_before_build() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("crate should live under repo root");
        let workflow = std::fs::read_to_string(repo_root.join(".github/workflows/ci.yml"))
            .expect("ci workflow should be readable");
        let test_step = workflow
            .find("run: npm test")
            .expect("web job should run npm test");
        let build_step = workflow
            .find("run: npm run build")
            .expect("web job should run npm build");
        assert!(
            test_step < build_step,
            "web job should run tests before build"
        );
    }
}
