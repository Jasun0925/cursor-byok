use super::*;

#[test]
fn collector_rejects_changed_roots_unrequested_bodies_and_duplicates() {
    let mut collector = Collector::new("/workspace".into(), vec![ContentType::Code]);
    let fingerprint = "a".repeat(64);
    let inventory = json!({"ok":true,"fingerprint":fingerprint,"inventory":{
    "root":"/workspace","rules":[],"entries":[
        {"path":"public.rs","is_dir":false,"size":3,"git_match":"none"},
        {"path":"private.rs","is_dir":false,"size":3,"git_match":"ignore"}
    ]}});
    let mut other = inventory.clone();
    other["inventory"]["root"] = json!("/other");
    assert!(collector.accept_payload(other).is_err());
    assert!(collector.accept_payload(inventory).unwrap().is_none());
    let request = collector.command().unwrap();
    assert!(request.len() < 60_000);
    let wrong =
        json!({"ok":true,"fingerprint":fingerprint,"files":[{"path":"private.rs","data":"YWJj"}]});
    assert!(collector.accept_payload(wrong).is_err());
    let correct =
        json!({"ok":true,"fingerprint":fingerprint,"files":[{"path":"public.rs","data":"YWJj"}]});
    let files = collector.accept_payload(correct).unwrap().unwrap();
    assert_eq!(files[0].bytes, b"abc");
}

#[cfg(any(unix, windows))]
mod host {
    use super::*;
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
    };

    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn write(root: &Path, path: &str, body: &str) {
        let file = root.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, body).unwrap();
    }

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let parent = tempfile::tempdir().unwrap();
        let canonical = parent.path().canonicalize().unwrap();
        // std::fs canonicalize uses extended Windows syntax; the protocol
        // carries ordinary absolute drive/UNC paths.
        let canonical = canonical.to_string_lossy();
        let root =
            PathBuf::from(canonical.strip_prefix("\\\\?\\").unwrap_or(&canonical)).join("repo");
        fs::create_dir(&root).unwrap();
        git(&root, &["init", "--quiet"]);
        (parent, root)
    }

    fn process(collector: &Collector) -> Command {
        let command = collector.command().unwrap();
        #[cfg(unix)]
        let mut process = {
            let mut p = Command::new("/bin/sh");
            p.arg("-c").arg(command);
            p.env("GIT_CONFIG_GLOBAL", "/dev/null");
            p
        };
        #[cfg(windows)]
        let mut process = {
            let mut parts = command.split_ascii_whitespace();
            let mut p = Command::new(parts.next().unwrap());
            p.args(parts);
            p.env("GIT_CONFIG_GLOBAL", "NUL");
            p
        };
        process
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .current_dir(&collector.root);
        process
    }

    fn run(collector: &Collector) -> std::process::Output {
        process(collector).output().unwrap()
    }

    #[test]
    fn real_host_does_not_import_workspace_python_modules() {
        let (_parent, root) = fixture();
        let body = "fn public() {}\n";
        write(&root, "public.rs", body);
        // These are excluded source files, never collector dependencies.
        for name in ["base64.py", "json.py", "sitecustomize.py"] {
            write(
                &root,
                name,
                "raise RuntimeError('WORKSPACE_CODE_EXECUTED')\n",
            );
        }
        write(&root, ".cursorignore", "*.py\n");
        let before = fs::read_dir(&root).unwrap().count();
        let mut collector =
            Collector::new(root.to_string_lossy().into_owned(), vec![ContentType::Code]);
        for stage in 0..2 {
            let output = run(&collector);
            assert!(
                output.status.success(),
                "stage={stage} stdout={} stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let result = collector
                .accept(std::str::from_utf8(&output.stdout).unwrap())
                .unwrap();
            if stage == 0 {
                assert!(result.is_none());
            } else {
                let files = result.unwrap();
                assert_eq!(files.len(), 1);
                assert_eq!(files[0].relative_path, "public.rs");
                assert_eq!(files[0].bytes, body.as_bytes());
            }
        }
        assert_eq!(fs::read_dir(&root).unwrap().count(), before);
        assert!(!root.join("__pycache__").exists());
    }

    #[test]
    fn real_host_does_not_import_modules_from_python_environment() {
        let (_parent, root) = fixture();
        write(&root, "public.rs", "fn public() {}\n");
        let imports = root.parent().unwrap().join("python-env");
        fs::create_dir(&imports).unwrap();
        write(
            &imports,
            "base64.py",
            "raise RuntimeError('ENVIRONMENT_CODE_EXECUTED')\n",
        );
        let mut collector =
            Collector::new(root.to_string_lossy().into_owned(), vec![ContentType::Code]);
        for stage in 0..2 {
            let output = process(&collector)
                .env("PYTHONPATH", &imports)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "stage={stage} stdout={} stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let result = collector
                .accept(std::str::from_utf8(&output.stdout).unwrap())
                .unwrap();
            assert_eq!(result.is_some(), stage == 1);
            if let Some(files) = result {
                assert_eq!(files.len(), 1);
                assert_eq!(files[0].relative_path, "public.rs");
            }
        }
        assert!(!imports.join("__pycache__").exists());
    }

    #[test]
    fn real_host_filters_before_body_transfer_and_applies_git_parent_and_nested_rules() {
        let (_parent, root) = fixture();
        let body = "fn allowed_snapshot_source() {}\n";
        for name in [
            "public.rs",
            "git.rs",
            "tracked.rs",
            "cursor.rs",
            "global.rs",
            "excluded.rs",
            "parent.rs",
            "nested/private.rs",
            "nested/public.rs",
        ] {
            write(&root, name, body);
        }
        write(&root, ".gitignore", "git.rs\ntracked.rs\n");
        write(&root, ".git/info/exclude", "excluded.rs\n");
        write(root.parent().unwrap(), "global.ignore", "global.rs\n");
        git(
            &root,
            &[
                "config",
                "core.excludesFile",
                root.parent()
                    .unwrap()
                    .join("global.ignore")
                    .to_str()
                    .unwrap(),
            ],
        );
        git(&root, &["add", "-f", "tracked.rs"]);
        write(root.parent().unwrap(), ".cursorignore", "repo/parent.rs\n");
        write(&root, ".cursorignore", "cursor.rs\nnested/*.rs\n");
        write(&root, "nested/.cursorignore", "!public.rs\n");
        write(&root, ".sembleignore", "!git.rs\n!cursor.rs\n");
        let mut collector =
            Collector::new(root.to_string_lossy().into_owned(), vec![ContentType::Code]);
        let inventory = run(&collector);
        assert!(
            inventory.status.success(),
            "{}",
            String::from_utf8_lossy(&inventory.stdout)
        );
        let text = String::from_utf8(inventory.stdout).unwrap();
        assert!(!text.contains("allowed_snapshot_source"));
        assert!(collector.accept(&text).unwrap().is_none());
        let output = run(&collector);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let files = collector
            .accept(std::str::from_utf8(&output.stdout).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(
            files
                .iter()
                .map(|f| f.relative_path.as_str())
                .collect::<Vec<_>>(),
            vec!["git.rs", "nested/public.rs", "public.rs"]
        );
        assert!(files.iter().all(|file| file.bytes == body.as_bytes()));
    }

    #[test]
    fn real_host_rejects_changes_between_inventory_and_body_collection() {
        let (_parent, root) = fixture();
        write(&root, "public.rs", "fn before() {}\n");
        let mut collector =
            Collector::new(root.to_string_lossy().into_owned(), vec![ContentType::Code]);
        let inventory = run(&collector);
        collector
            .accept(std::str::from_utf8(&inventory.stdout).unwrap())
            .unwrap();
        write(&root, ".cursorignore", "public.rs\n");
        let output = run(&collector);
        assert!(!output.status.success());
        let text = std::str::from_utf8(&output.stdout).unwrap();
        assert!(!text.contains("fn before"));
        assert!(collector.accept(text).unwrap_err().contains("changed"));
    }

    #[test]
    #[cfg(unix)]
    fn real_host_skips_links_and_preserves_binary_bytes_without_following_paths() {
        use std::os::unix::fs::symlink;
        let (_parent, root) = fixture();
        write(root.parent().unwrap(), "outside.rs", "OUTSIDE_ONLY");
        symlink(
            root.parent().unwrap().join("outside.rs"),
            root.join("linked.rs"),
        )
        .unwrap();
        symlink(root.parent().unwrap(), root.join("linked-dir")).unwrap();
        fs::write(root.join("source.rs"), b"fn source() { /* \xff */ }\r\n").unwrap();
        let mut collector =
            Collector::new(root.to_string_lossy().into_owned(), vec![ContentType::Code]);
        let inventory = run(&collector);
        assert!(inventory.status.success());
        collector
            .accept(std::str::from_utf8(&inventory.stdout).unwrap())
            .unwrap();
        let output = run(&collector);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let files = collector
            .accept(std::str::from_utf8(&output.stdout).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, "source.rs");
        assert_eq!(files[0].bytes, b"fn source() { /* \xff */ }\r\n");
    }

    #[test]
    fn real_host_batches_unicode_names_and_preserves_raw_bytes() {
        let (_parent, root) = fixture();
        let bytes = b"fn source() { /* \xff */ }\r\n";
        for i in 0..140 {
            fs::write(root.join(format!("source_{i:03}_中文.rs")), bytes).unwrap();
        }
        let mut collector =
            Collector::new(root.to_string_lossy().into_owned(), vec![ContentType::Code]);
        let mut rounds = 0;
        let files = loop {
            let output = run(&collector);
            assert!(
                output.status.success(),
                "stdout={} stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            rounds += 1;
            if let Some(files) = collector
                .accept(std::str::from_utf8(&output.stdout).unwrap())
                .unwrap()
            {
                break files;
            }
            assert!(rounds < 20);
        };
        assert!(rounds >= 3, "inventory and at least two body batches");
        assert_eq!(files.len(), 140);
        assert!(files.iter().all(|f| f.bytes == bytes));
        assert_eq!(
            files
                .iter()
                .map(|f| &f.relative_path)
                .collect::<HashSet<_>>()
                .len(),
            140
        );
    }

    #[test]
    fn real_host_pages_large_bodies_below_cursor_inline_output_limit() {
        let (_parent, root) = fixture();
        let bytes = vec![b'x'; 950_000];
        fs::write(root.join("large.rs"), &bytes).unwrap();
        let mut collector =
            Collector::new(root.to_string_lossy().into_owned(), vec![ContentType::Code]);
        let mut rounds = 0;
        let files = loop {
            let output = run(&collector);
            assert!(output.status.success());
            assert!(
                output.stdout.len() < 512 * 1024,
                "a single Exec must stay below Cursor's 1 MiB inline limit: {} bytes",
                output.stdout.len()
            );
            rounds += 1;
            if let Some(files) = collector
                .accept(std::str::from_utf8(&output.stdout).unwrap())
                .unwrap()
            {
                break files;
            }
            assert!(rounds < 20);
        };
        assert!(rounds > 2, "large source must use multiple response pages");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].bytes, bytes);
    }

    #[test]
    fn real_host_pages_large_inventory_without_reducing_workspace_capacity() {
        let (_parent, root) = fixture();
        for i in 0..5000 {
            fs::write(
                root.join(format!("file_{i:04}_{}.rs", "n".repeat(180))),
                b"fn f() {}",
            )
            .unwrap();
        }
        let mut collector =
            Collector::new(root.to_string_lossy().into_owned(), vec![ContentType::Code]);
        let mut rounds = 0;
        while matches!(collector.phase, Phase::Inventory) {
            let output = run(&collector);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stdout.len() < 512 * 1024);
            assert!(collector
                .accept(std::str::from_utf8(&output.stdout).unwrap())
                .unwrap()
                .is_none());
            rounds += 1;
            assert!(rounds < 20);
        }
        assert!(rounds > 1);
        let Phase::Reading {
            requested,
            remaining,
            ..
        } = collector.phase
        else {
            panic!()
        };
        assert_eq!(
            requested.len() + remaining.iter().map(Vec::len).sum::<usize>(),
            5000
        );
    }

    #[test]
    fn real_host_rejects_changed_large_response_between_pages() {
        let (_parent, root) = fixture();
        fs::write(root.join("large.rs"), vec![b'x'; 950_000]).unwrap();
        let mut collector =
            Collector::new(root.to_string_lossy().into_owned(), vec![ContentType::Code]);
        for _ in 0..2 {
            let output = run(&collector);
            assert!(output.status.success());
            assert!(collector
                .accept(std::str::from_utf8(&output.stdout).unwrap())
                .unwrap()
                .is_none());
        }
        fs::write(root.join("large.rs"), vec![b'y'; 950_000]).unwrap();
        let output = run(&collector);
        assert!(!output.status.success());
        assert!(collector
            .accept(std::str::from_utf8(&output.stdout).unwrap())
            .unwrap_err()
            .contains("changed"));
    }

    #[test]
    fn real_host_uses_nested_git_rules_and_rejects_excluded_workspace_roots() {
        let (_parent, root) = fixture();
        write(&root, "nested/public.rs", "fn public() {}\n");
        write(&root, "nested/private.rs", "PRIVATE_BODY_MUST_NOT_TRANSFER");
        git(&root.join("nested"), &["init", "--quiet"]);
        write(&root, "nested/.gitignore", "private.rs\n");
        let mut collector =
            Collector::new(root.to_string_lossy().into_owned(), vec![ContentType::Code]);
        let inventory = run(&collector);
        assert!(inventory.status.success());
        collector
            .accept(std::str::from_utf8(&inventory.stdout).unwrap())
            .unwrap();
        let output = run(&collector);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let files = collector
            .accept(std::str::from_utf8(&output.stdout).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, "nested/public.rs");
        write(root.parent().unwrap(), ".cursorignore", "repo/\n");
        let mut blocked = Collector::new(
            root.join("nested").to_string_lossy().into_owned(),
            vec![ContentType::Code],
        );
        let inventory = run(&blocked);
        assert!(inventory.status.success());
        assert!(blocked
            .accept(std::str::from_utf8(&inventory.stdout).unwrap())
            .unwrap_err()
            .contains("No allowed"));
    }

    #[test]
    #[cfg(windows)]
    fn real_windows_skips_junctions_and_rejects_roots_with_junction_ancestors() {
        let (_parent, root) = fixture();
        write(&root, "public.rs", "fn public() {}\n");
        let outside = root.parent().unwrap().join("outside");
        fs::create_dir(&outside).unwrap();
        write(&outside, "private.rs", "OUTSIDE_BODY");
        write(&outside, "nested/private.rs", "OUTSIDE_NESTED_BODY");
        let link = root.join("link");
        let result = Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let mut collector =
            Collector::new(root.to_string_lossy().into_owned(), vec![ContentType::Code]);
        let inventory = run(&collector);
        assert!(
            inventory.status.success(),
            "{}",
            String::from_utf8_lossy(&inventory.stdout)
        );
        collector
            .accept(std::str::from_utf8(&inventory.stdout).unwrap())
            .unwrap();
        let output = run(&collector);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let files = collector
            .accept(std::str::from_utf8(&output.stdout).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, "public.rs");
        let alias = Collector::new(
            link.join("nested").to_string_lossy().into_owned(),
            vec![ContentType::Code],
        );
        let denied = run(&alias);
        assert!(!denied.status.success());
        assert!(!String::from_utf8_lossy(&denied.stdout).contains("OUTSIDE_NESTED_BODY"));
    }
}

#[test]
fn roots_and_launchers_follow_host_syntax_without_shell_interpolation() {
    for root in [
        "/workspace",
        "C:/workspace",
        "C:/",
        "//server/share/workspace",
    ] {
        assert!(valid_root(root), "{root}");
        let collector = Collector::new(root.into(), vec![ContentType::Code]);
        let command = collector.command().unwrap();
        assert_eq!(command.starts_with("powershell.exe"), windows_root(root));
        assert!(command.len() < if windows_root(root) { 32_000 } else { 60_000 });
    }
    for root in [
        "workspace",
        "C:workspace",
        "/workspace/../outside",
        "//server",
        "/workspace//dir",
    ] {
        assert!(!valid_root(root), "{root}");
    }
    assert_eq!(normalize_root("C:\\workspace\\"), "C:/workspace");
    assert_eq!(normalize_root("C:\\"), "C:/");
}
