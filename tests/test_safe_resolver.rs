#![cfg(all(feature = "serialize", feature = "deserialize"))]
#![cfg(all(feature = "include_fs", not(miri), not(target_os = "wasi")))]

use serde::Deserialize;
use serde_saphyr::{
    IncludeRequest, InputSource, Location, Options, SafeFileReadMode, SafeFileResolver,
    SymlinkPolicy, from_str_with_options,
};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

#[derive(Debug, Deserialize, PartialEq)]
struct ScalarConfig {
    foo: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct NestedConfig {
    foo: InnerConfig,
}

#[derive(Debug, Deserialize, PartialEq)]
struct UsersConfig {
    foo: Vec<User>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct AnchoredUsersConfig {
    selected_users: Vec<User>,
    repeated_users: Vec<User>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct InnerConfig {
    bar: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct User {
    id: u32,
    name: String,
    roles: Vec<String>,
}

fn request<'a>(spec: &'a str, from_name: &'a str, from_id: Option<&'a str>) -> IncludeRequest<'a> {
    IncludeRequest {
        spec,
        from_name,
        from_id,
        stack: Vec::new(),
        size_remaining: None,
        location: Location::UNKNOWN,
    }
}

fn include_error_message(err: serde_saphyr::IncludeResolveError) -> String {
    match err {
        serde_saphyr::IncludeResolveError::Message(msg) => msg,
        serde_saphyr::IncludeResolveError::Io(e) => e.to_string(),
        serde_saphyr::IncludeResolveError::SizeLimitExceeded(size, limit) => {
            format!("include size {size} bytes exceeds remaining size limit {limit} bytes")
        }
        serde_saphyr::IncludeResolveError::FileInclude(problem) => match &*problem {
            serde_saphyr::ResolveProblem::ResolveFailed {
                spec,
                base_dir,
                err,
            } => {
                format!(
                    "failed to resolve include '{}' from '{}': {}",
                    spec, base_dir, err
                )
            }
            serde_saphyr::ResolveProblem::TargetNotRegularFile { target } => {
                format!("include target '{}' is not a regular file", target)
            }
            serde_saphyr::ResolveProblem::TargetIsRootFile { spec } => {
                format!(
                    "include target '{}' resolves to the configured root file itself",
                    spec
                )
            }
            serde_saphyr::ResolveProblem::ParentIdNotAbsoluteCanonical { parent_id } => {
                format!(
                    "SafeFileResolver expected parent include id to be an absolute canonical path, got '{}'",
                    parent_id
                )
            }
            serde_saphyr::ResolveProblem::ParentResolveFailed {
                parent_id,
                from_name,
                err,
            } => {
                format!(
                    "failed to resolve parent include source '{}' (from '{}'): {}",
                    parent_id, from_name, err
                )
            }
            serde_saphyr::ResolveProblem::ParentNotRegularFile { parent } => {
                format!("include parent '{}' is not a regular file", parent)
            }
            serde_saphyr::ResolveProblem::ParentHasNoDirectory { parent } => {
                format!(
                    "include parent '{}' does not have a parent directory",
                    parent
                )
            }
            serde_saphyr::ResolveProblem::ResolvesOutsideRoot { spec, root } => {
                format!(
                    "include '{}' resolves outside the configured root '{}'",
                    spec, root
                )
            }
            serde_saphyr::ResolveProblem::TraversesSymlink { spec } => {
                format!(
                    "include '{}' traverses a symlink, which is disabled by policy",
                    spec
                )
            }
            serde_saphyr::ResolveProblem::AbsolutePathNotAllowed { spec } => {
                format!("absolute include paths are not allowed: {}", spec)
            }
            serde_saphyr::ResolveProblem::EmptyPath => "include path must not be empty".to_string(),
            serde_saphyr::ResolveProblem::InvalidExtension { spec } => {
                format!(
                    "include target '{}' does not have a valid YAML extension (.yml or .yaml)",
                    spec
                )
            }
            serde_saphyr::ResolveProblem::HiddenFile { spec } => {
                format!(
                    "include target '{}' is a hidden file, which is not allowed",
                    spec
                )
            }
            serde_saphyr::ResolveProblem::EmptyFragment => {
                "include fragment must not be empty".to_string()
            }
            serde_saphyr::ResolveProblem::FragmentContainsHash { spec } => {
                format!("include fragment must not contain '#': {}", spec)
            }
            _ => "unknown resolve problem".to_string(),
        },
        _ => "unknown error".to_string(),
    }
}

fn write_text(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, text).unwrap();
}

fn write_utf16le(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[0xFF, 0xFE]);
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    fs::write(path, bytes).unwrap();
}

#[test]
fn safe_file_resolver_rejects_invalid_extension() {
    let temp = TempDir::new().unwrap();
    write_text(&temp.path().join("value.txt"), "bar_value\n");

    let options = Options::default()
        .with_include_resolver(SafeFileResolver::new(temp.path()).unwrap().into_callback());

    let err =
        from_str_with_options::<ScalarConfig>("foo: !include value.txt\n", options).unwrap_err();
    let inner_err = match err {
        serde_saphyr::Error::WithSnippet { error, .. } => *error,
        e => e,
    };
    let msg = include_error_message(match inner_err {
        serde_saphyr::Error::ResolverError { error, .. } => error,
        e => panic!("expected ResolverError, got {:?}", e),
    });
    assert!(msg.contains("does not have a valid YAML extension"));
}

#[test]
fn safe_file_resolver_rejects_hidden_files() {
    let temp = TempDir::new().unwrap();
    write_text(&temp.path().join(".hidden.yml"), "bar_value\n");

    let options = Options::default()
        .with_include_resolver(SafeFileResolver::new(temp.path()).unwrap().into_callback());

    let err =
        from_str_with_options::<ScalarConfig>("foo: !include .hidden.yml\n", options).unwrap_err();
    let inner_err = match err {
        serde_saphyr::Error::WithSnippet { error, .. } => *error,
        e => e,
    };
    let msg = include_error_message(match inner_err {
        serde_saphyr::Error::ResolverError { error, .. } => error,
        e => panic!("expected ResolverError, got {:?}", e),
    });
    assert!(msg.contains("is a hidden file, which is not allowed"));
}

#[test]
fn safe_file_resolver_rejects_files_inside_hidden_directories() {
    let temp = TempDir::new().unwrap();
    write_text(&temp.path().join(".hidden/value.yaml"), "bar_value\n");

    let options = Options::default()
        .with_include_resolver(SafeFileResolver::new(temp.path()).unwrap().into_callback());

    let err = from_str_with_options::<ScalarConfig>("foo: !include .hidden/value.yaml\n", options)
        .unwrap_err();
    let inner_err = match err {
        serde_saphyr::Error::WithSnippet { error, .. } => *error,
        e => e,
    };
    let msg = include_error_message(match inner_err {
        serde_saphyr::Error::ResolverError { error, .. } => error,
        e => panic!("expected ResolverError, got {:?}", e),
    });
    assert!(msg.contains("is a hidden file, which is not allowed"));
}

#[test]
fn safe_file_resolver_top_level_relative() {
    let temp = TempDir::new().unwrap();
    write_text(&temp.path().join("value.yaml"), "bar_value\n");

    let options = Options::default()
        .with_include_resolver(SafeFileResolver::new(temp.path()).unwrap().into_callback());

    let parsed: ScalarConfig =
        from_str_with_options("foo: !include value.yaml\n", options).unwrap();
    assert_eq!(parsed.foo, "bar_value");
}

#[test]
fn options_with_filesystem_root_uses_safe_file_resolver() {
    let temp = TempDir::new().unwrap();
    write_text(&temp.path().join("value.yaml"), "bar_value\n");

    let options = Options::default()
        .with_filesystem_root(temp.path())
        .unwrap();

    let parsed: ScalarConfig =
        from_str_with_options("foo: !include value.yaml\n", options).unwrap();
    assert_eq!(parsed.foo, "bar_value");
}

#[test]
fn safe_file_resolver_supports_path_fragment_syntax() {
    let temp = TempDir::new().unwrap();
    write_text(
        &temp.path().join("value.yaml"),
        "users: &users\n  - id: 1\n    name: Alice\n    roles: [admin]\n  - id: 2\n    name: Bob\n    roles: [viewer]\n",
    );

    let resolver = SafeFileResolver::new(temp.path()).unwrap();
    let resolved = resolver
        .resolve(request("value.yaml#users", "", None))
        .unwrap();
    match resolved.source {
        InputSource::AnchoredText { text, anchor } => {
            assert_eq!(anchor, "users");
            assert!(text.contains("users: &users"));
            assert!(text.contains("name: Alice"));
            assert!(text.contains("name: Bob"));
        }
        InputSource::Text(text) => assert_eq!(
            text,
            "- id: 1\n  name: Alice\n  roles: [admin]\n- id: 2\n  name: Bob\n  roles: [viewer]"
        ),
        InputSource::Reader(_) => panic!("fragment include should be materialized as text"),
        _ => panic!("unexpected future InputSource variant"),
    }

    let options = Options::default()
        .with_filesystem_root(temp.path())
        .unwrap();

    let parsed: UsersConfig =
        from_str_with_options("foo: !include value.yaml#users\n", options).unwrap();
    assert_eq!(parsed.foo.len(), 2);
    assert_eq!(parsed.foo[0].name, "Alice");
    assert_eq!(parsed.foo[1].name, "Bob");
}

#[test]
fn safe_file_resolver_preserves_fragment_anchor_for_aliases() {
    let temp = TempDir::new().unwrap();
    write_text(
        &temp.path().join("value.yaml"),
        "users: &users\n  - id: 1\n    name: Alice\n    roles: [admin]\n  - id: 2\n    name: Bob\n    roles: [viewer]\n",
    );

    let options = Options::default()
        .with_filesystem_root(temp.path())
        .unwrap();

    let parsed: AnchoredUsersConfig = from_str_with_options(
        "selected_users: &users !include#users value.yaml\nrepeated_users: *users\n",
        options,
    )
    .unwrap();
    assert_eq!(parsed.selected_users, parsed.repeated_users);
}

#[test]
fn safe_file_resolver_nested_relative_from_parent_id() {
    let temp = TempDir::new().unwrap();
    write_text(
        &temp.path().join("env/prod.yaml"),
        "bar: !include ../shared/value.yaml\n",
    );
    write_text(&temp.path().join("shared/value.yaml"), "nested_value\n");

    let options = Options::default()
        .with_include_resolver(SafeFileResolver::new(temp.path()).unwrap().into_callback());

    let parsed: NestedConfig =
        from_str_with_options("foo: !include env/prod.yaml\n", options).unwrap();
    assert_eq!(parsed.foo.bar, "nested_value");
}

#[test]
fn safe_file_resolver_uses_root_file_parent_for_top_level_relative_includes() {
    let temp = TempDir::new().unwrap();
    let allow_root = temp.path().join("config");
    let root_file = allow_root.join("env/prod/root.yaml");
    write_text(&root_file, "foo: !include ../common/value.yaml\n");
    write_text(
        &allow_root.join("env/common/value.yaml"),
        "from_root_file\n",
    );

    let resolver = SafeFileResolver::new(&allow_root)
        .unwrap()
        .with_root_file(&root_file)
        .unwrap();
    let options = Options::default().with_include_resolver(resolver.into_callback());

    let parsed: ScalarConfig =
        from_str_with_options("foo: !include ../common/value.yaml\n", options).unwrap();
    assert_eq!(parsed.foo, "from_root_file");
}

#[test]
fn safe_file_resolver_rejects_escape() {
    let temp = TempDir::new().unwrap();
    let allow_root = temp.path().join("allowed");
    fs::create_dir_all(&allow_root).unwrap();
    write_text(&temp.path().join("outside.yaml"), "outside\n");

    let resolver = SafeFileResolver::new(&allow_root).unwrap();
    let err = resolver
        .resolve(request("../outside.yaml", "", None))
        .unwrap_err();
    let msg = include_error_message(err);
    assert!(msg.contains("outside the configured root"), "{}", msg);
}

#[test]
fn safe_file_resolver_rejects_absolute_paths() {
    let temp = TempDir::new().unwrap();
    let allow_root = temp.path().join("allowed");
    fs::create_dir_all(&allow_root).unwrap();
    let absolute_target = temp.path().join("absolute.yaml");
    write_text(&absolute_target, "absolute\n");

    let resolver = SafeFileResolver::new(&allow_root).unwrap();
    let spec = absolute_target.to_string_lossy().into_owned();
    let err = resolver.resolve(request(&spec, "", None));
    assert!(err.is_err());
}

#[test]
fn safe_file_resolver_reports_missing_fragment() {
    let temp = TempDir::new().unwrap();
    write_text(&temp.path().join("value.yaml"), "users: &users []\n");

    let options = Options::default()
        .with_filesystem_root(temp.path())
        .unwrap();
    let err = from_str_with_options::<ScalarConfig>("foo: !include value.yaml#section\n", options)
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("fragment 'section' was not found"), "{}", msg);
}

#[test]
fn safe_file_resolver_rejects_self_include() {
    let temp = TempDir::new().unwrap();
    let root_file = temp.path().join("root.yaml");
    write_text(&root_file, "foo: !include root.yaml\n");

    let resolver = SafeFileResolver::new(temp.path())
        .unwrap()
        .with_root_file(&root_file)
        .unwrap();
    let err = resolver
        .resolve(request("root.yaml", "", None))
        .unwrap_err();
    let msg = include_error_message(err);
    assert!(msg.contains("configured root file itself"), "{}", msg);
}

#[test]
fn safe_file_resolver_text_mode_decodes_bom() {
    let temp = TempDir::new().unwrap();
    write_utf16le(&temp.path().join("value.yaml"), "bar_value\n");

    let resolver = SafeFileResolver::new(temp.path())
        .unwrap()
        .with_read_mode(SafeFileReadMode::Text);
    let options = Options::default().with_include_resolver(resolver.into_callback());

    let parsed: ScalarConfig =
        from_str_with_options("foo: !include value.yaml\n", options).unwrap();
    assert_eq!(parsed.foo, "bar_value");
}

#[test]
fn safe_file_resolver_streaming_mode_still_works() {
    let temp = TempDir::new().unwrap();
    write_text(&temp.path().join("value.yaml"), "bar_value\n");

    let resolver = SafeFileResolver::new(temp.path())
        .unwrap()
        .with_read_mode(SafeFileReadMode::Reader);
    let options = Options::default().with_include_resolver(resolver.into_callback());

    let parsed: ScalarConfig =
        from_str_with_options("foo: !include value.yaml\n", options).unwrap();
    assert_eq!(parsed.foo, "bar_value");
}

#[cfg(unix)]
#[test]
fn safe_file_resolver_default_symlink_policy_is_reject() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();
    let target = temp.path().join("allowed/real.yaml");
    write_text(&target, "bar_value\n");
    symlink(&target, temp.path().join("allowed/link.yaml")).unwrap();

    let resolver = SafeFileResolver::new(temp.path().join("allowed")).unwrap();
    let err = resolver
        .resolve(request("link.yaml", "", None))
        .unwrap_err();
    let msg = include_error_message(err);
    assert!(msg.contains("traverses a symlink"), "{}", msg);
}

#[cfg(unix)]
#[test]
fn safe_file_resolver_symlink_policy_follow_within_root() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();
    let target = temp.path().join("allowed/real.yaml");
    write_text(&target, "bar_value\n");
    symlink(&target, temp.path().join("allowed/link.yaml")).unwrap();

    let resolver = SafeFileResolver::new(temp.path().join("allowed"))
        .unwrap()
        .with_symlink_policy(SymlinkPolicy::FollowWithinRoot);
    let options = Options::default().with_include_resolver(resolver.into_callback());

    let parsed: ScalarConfig = from_str_with_options("foo: !include link.yaml\n", options).unwrap();
    assert_eq!(parsed.foo, "bar_value");
}

#[cfg(unix)]
#[test]
fn safe_file_resolver_no_git() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();

    // Case 1: real path contains .ggit, linked path does not
    let target1 = temp.path().join("allowed/.ggit/real.yaml");
    write_text(&target1, "bar_value\n");
    symlink(&target1, temp.path().join("allowed/link1.yaml")).unwrap();

    // Case 2: linked path contains .ggit
    let target2 = temp.path().join("allowed/real2.yaml");
    write_text(&target2, "bar_value\n");
    let link2_dir = temp.path().join("allowed/.ggit");
    std::fs::create_dir_all(&link2_dir).unwrap();
    symlink(&target2, link2_dir.join("link2.yaml")).unwrap();

    let resolver = SafeFileResolver::new(temp.path().join("allowed"))
        .unwrap()
        .with_symlink_policy(SymlinkPolicy::FollowWithinRoot);

    // Test Case 1
    let options1 = Options::default().with_include_resolver(resolver.clone().into_callback());
    let parsed1 = from_str_with_options::<ScalarConfig>("foo: !include link1.yaml\n", options1);
    assert!(parsed1.is_err());

    // Test Case 2
    let options2 = Options::default().with_include_resolver(resolver.into_callback());
    let parsed2 =
        from_str_with_options::<ScalarConfig>("foo: !include .ggit/link2.yaml\n", options2);
    assert!(parsed2.is_err());
}

#[cfg(unix)]
#[test]
fn safe_file_resolver_rejects_symlink_escape_even_when_following_symlinks() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();
    let allow_root = temp.path().join("allowed");
    fs::create_dir_all(&allow_root).unwrap();
    let outside = temp.path().join("outside.yaml");
    write_text(&outside, "outside\n");
    symlink(&outside, allow_root.join("link.yaml")).unwrap();

    let resolver = SafeFileResolver::new(&allow_root)
        .unwrap()
        .with_symlink_policy(SymlinkPolicy::FollowWithinRoot);
    let err = resolver
        .resolve(request("link.yaml", "", None))
        .unwrap_err();
    let msg = include_error_message(err);
    assert!(msg.contains("outside the configured root"), "{}", msg);
}

#[cfg(unix)]
#[test]
fn safe_file_resolver_symlink_policy_reject() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();
    let target = temp.path().join("allowed/real.yaml");
    write_text(&target, "bar_value\n");
    symlink(&target, temp.path().join("allowed/link.yaml")).unwrap();

    let resolver = SafeFileResolver::new(temp.path().join("allowed"))
        .unwrap()
        .with_symlink_policy(SymlinkPolicy::Reject);
    let err = resolver
        .resolve(request("link.yaml", "", None))
        .unwrap_err();
    let msg = include_error_message(err);
    assert!(msg.contains("traverses a symlink"), "{}", msg);
}
