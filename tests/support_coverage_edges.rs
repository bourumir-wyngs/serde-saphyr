#![cfg(feature = "deserialize")]

use serde::Deserialize;
use serde::de::value::{Error as ValueError, SeqDeserializer};
use serde_saphyr::{
    ArcAnchor, ArcRecursion, ArcWeakAnchor, Commented, RcAnchor, RcRecursion, RcWeakAnchor,
};

fn expect_error<T, E>(result: Result<T, E>) -> E {
    match result {
        Ok(_) => panic!("operation unexpectedly succeeded"),
        Err(error) => error,
    }
}

#[test]
fn commented_supports_non_yaml_newtypes_and_rejects_empty_sequences() {
    let value: Commented<u32> = serde_json::from_str("5").unwrap();
    assert_eq!(value, Commented(5, String::new()));

    let empty = SeqDeserializer::<_, ValueError>::new(std::iter::empty::<u8>());
    assert!(Commented::<u8>::deserialize(empty).is_err());
}

#[cfg(feature = "validator")]
#[test]
fn commented_forwards_validator_arguments() {
    struct RequiresArgument(i32);

    impl<'a> validator::ValidateArgs<'a> for RequiresArgument {
        type Args = i32;

        fn validate_with_args(
            &self,
            argument: Self::Args,
        ) -> Result<(), validator::ValidationErrors> {
            assert_eq!(argument, self.0);
            Ok(())
        }
    }

    let value = Commented(RequiresArgument(41), "checked".to_owned());
    validator::ValidateArgs::validate_with_args(&value, 41).unwrap();
}

#[test]
fn recursive_strong_anchor_types_are_rejected_at_the_cycle() {
    #[derive(Deserialize)]
    struct RcNode {
        #[allow(dead_code)]
        next: RcAnchor<RcNode>,
    }

    #[derive(Deserialize)]
    struct RcDoc {
        #[allow(dead_code)]
        root: RcAnchor<RcNode>,
    }

    #[derive(Deserialize)]
    struct ArcNode {
        #[allow(dead_code)]
        next: ArcAnchor<ArcNode>,
    }

    #[derive(Deserialize)]
    struct ArcDoc {
        #[allow(dead_code)]
        root: ArcAnchor<ArcNode>,
    }

    let yaml = "root: &loop\n  next: *loop\n";
    let rc_error = expect_error(serde_saphyr::from_str::<RcDoc>(yaml));
    let arc_error = expect_error(serde_saphyr::from_str::<ArcDoc>(yaml));
    assert!(
        matches!(
            rc_error.without_snippet(),
            serde_saphyr::Error::RecursiveReferencesRequireWeakTypes { .. }
        ),
        "{rc_error:?}"
    );
    assert!(
        matches!(
            arc_error.without_snippet(),
            serde_saphyr::Error::RecursiveReferencesRequireWeakTypes { .. }
        ),
        "{arc_error:?}"
    );
}

#[test]
fn weak_anchor_types_distinguish_recursive_and_unknown_strong_targets() {
    #[derive(Deserialize)]
    struct RcNode {
        #[allow(dead_code)]
        next: RcWeakAnchor<RcNode>,
    }

    #[derive(Deserialize)]
    struct RcDoc {
        #[allow(dead_code)]
        root: RcAnchor<RcNode>,
    }

    #[derive(Deserialize)]
    struct ArcNode {
        #[allow(dead_code)]
        next: ArcWeakAnchor<ArcNode>,
    }

    #[derive(Deserialize)]
    struct ArcDoc {
        #[allow(dead_code)]
        root: ArcAnchor<ArcNode>,
    }

    let recursive = "root: &loop\n  next: *loop\n";
    let rc_error = expect_error(serde_saphyr::from_str::<RcDoc>(recursive));
    let arc_error = expect_error(serde_saphyr::from_str::<ArcDoc>(recursive));
    assert!(
        matches!(
            rc_error.without_snippet(),
            serde_saphyr::Error::RecursiveReferencesRequireWeakTypes { .. }
        ),
        "{rc_error:?}"
    );
    assert!(
        matches!(
            arc_error.without_snippet(),
            serde_saphyr::Error::RecursiveReferencesRequireWeakTypes { .. }
        ),
        "{arc_error:?}"
    );

    #[derive(Deserialize)]
    struct Value {
        #[allow(dead_code)]
        n: u8,
    }

    #[derive(Deserialize)]
    struct RcUnknownDoc {
        #[allow(dead_code)]
        value: Value,
        #[allow(dead_code)]
        weak: RcWeakAnchor<Value>,
    }

    #[derive(Deserialize)]
    struct ArcUnknownDoc {
        #[allow(dead_code)]
        value: Value,
        #[allow(dead_code)]
        weak: ArcWeakAnchor<Value>,
    }

    let unknown = "value: &plain {n: 1}\nweak: *plain\n";
    let rc_error = expect_error(serde_saphyr::from_str::<RcUnknownDoc>(unknown));
    let arc_error = expect_error(serde_saphyr::from_str::<ArcUnknownDoc>(unknown));
    assert!(matches!(
        rc_error.without_snippet(),
        serde_saphyr::Error::AliasError { .. }
    ));
    assert!(matches!(
        arc_error.without_snippet(),
        serde_saphyr::Error::AliasError { .. }
    ));
}

#[test]
fn recursion_alias_types_require_recursive_anchor_context_and_storage() {
    #[derive(Deserialize)]
    struct Value {
        #[allow(dead_code)]
        n: u8,
    }

    let rc_error = serde_saphyr::from_str::<RcRecursion<Value>>("{n: 1}").unwrap_err();
    assert!(matches!(
        rc_error.without_snippet(),
        serde_saphyr::Error::Message { .. }
    ));
    let arc_error = serde_saphyr::from_str::<ArcRecursion<Value>>("{n: 1}").unwrap_err();
    assert!(matches!(
        arc_error.without_snippet(),
        serde_saphyr::Error::Message { .. }
    ));

    #[derive(Deserialize)]
    struct RcDoc {
        #[allow(dead_code)]
        value: Value,
        #[allow(dead_code)]
        recursion: RcRecursion<Value>,
    }

    #[derive(Deserialize)]
    struct ArcDoc {
        #[allow(dead_code)]
        value: Value,
        #[allow(dead_code)]
        recursion: ArcRecursion<Value>,
    }

    let yaml = "value: &plain {n: 1}\nrecursion: *plain\n";
    let rc_error = expect_error(serde_saphyr::from_str::<RcDoc>(yaml));
    assert!(matches!(
        rc_error.without_snippet(),
        serde_saphyr::Error::AliasError { .. }
    ));
    let arc_error = expect_error(serde_saphyr::from_str::<ArcDoc>(yaml));
    assert!(matches!(
        arc_error.without_snippet(),
        serde_saphyr::Error::AliasError { .. }
    ));
}

#[cfg(all(feature = "include_fs", not(target_os = "wasi")))]
mod safe_resolver {
    use std::fs;
    use std::path::Path;

    use serde_saphyr::{
        IncludeRequest, IncludeResolveError, InputSource, Location, ResolveProblem,
        SafeFileResolver,
    };
    use tempfile::TempDir;

    fn write(path: &Path, text: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }

    fn request<'a>(spec: &'a str, from_id: Option<&'a str>) -> IncludeRequest<'a> {
        let request = IncludeRequest::new(spec, "parent.yaml", Location::UNKNOWN);
        match from_id {
            Some(from_id) => request.with_from_id(from_id),
            None => request,
        }
    }

    fn problem(error: &IncludeResolveError) -> &ResolveProblem {
        match error {
            IncludeResolveError::FileInclude(problem) => problem,
            other => panic!("expected filesystem resolver problem, got {other:?}"),
        }
    }

    #[test]
    fn root_configuration_checks_types_boundaries_and_reset_behavior() {
        let temp = TempDir::new().unwrap();
        let allowed = temp.path().join("allowed");
        let nested = allowed.join("nested");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let root_file = allowed.join("root.yaml");
        let nested_root = nested.join("root.yaml");
        let outside_file = outside.join("outside.yaml");
        write(&root_file, "root\n");
        write(&nested_root, "nested\n");
        write(&outside_file, "outside\n");

        assert_eq!(
            SafeFileResolver::new(&root_file).unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );

        let resolver = SafeFileResolver::new(&allowed).unwrap();
        assert_eq!(
            resolver
                .clone()
                .with_root_base_dir(&root_file)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
        assert_eq!(
            resolver.clone().with_root_file(&nested).unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
        assert_eq!(
            resolver
                .clone()
                .with_root_base_dir(&outside)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            resolver
                .clone()
                .with_root_file(&outside_file)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );

        let resolver = resolver
            .with_root_file(&root_file)
            .unwrap()
            .with_root_base_dir(&nested)
            .unwrap();
        let resolved = resolver.resolve(request("root.yaml", None)).unwrap();
        assert_eq!(
            resolved.name,
            Path::new("nested").join("root.yaml").display().to_string()
        );
        assert!(matches!(resolved.source, InputSource::Reader(_)));
    }

    #[test]
    fn malformed_specs_and_missing_targets_report_specific_problems() {
        let temp = TempDir::new().unwrap();
        write(&temp.path().join("child.yaml"), "value\n");
        let resolver = SafeFileResolver::new(temp.path()).unwrap();

        let empty = resolver.resolve(request("", None)).unwrap_err();
        assert!(matches!(problem(&empty), ResolveProblem::EmptyPath));

        let fragment_only = resolver.resolve(request("#anchor", None)).unwrap_err();
        assert!(matches!(problem(&fragment_only), ResolveProblem::EmptyPath));

        let empty_fragment = resolver.resolve(request("child.yaml#", None)).unwrap_err();
        assert!(matches!(
            problem(&empty_fragment),
            ResolveProblem::EmptyFragment
        ));

        let extra_hash = resolver
            .resolve(request("child.yaml#a#b", None))
            .unwrap_err();
        assert!(matches!(
            problem(&extra_hash),
            ResolveProblem::FragmentContainsHash { .. }
        ));

        let absolute = resolver
            .resolve(request("/definitely-missing-support-coverage.yaml", None))
            .unwrap_err();
        assert!(matches!(
            problem(&absolute),
            ResolveProblem::AbsolutePathNotAllowed { .. }
        ));

        let missing = resolver.resolve(request("missing.yaml", None)).unwrap_err();
        assert!(matches!(
            problem(&missing),
            ResolveProblem::ResolveFailed { .. }
        ));

        let resolved = resolver.resolve(request("./child.yaml", None)).unwrap();
        assert_eq!(resolved.name, "child.yaml");
    }

    #[test]
    fn target_and_parent_file_type_errors_are_distinct() {
        let temp = TempDir::new().unwrap();
        let target_dir = temp.path().join("directory.yaml");
        let parent_dir = temp.path().join("parent.yaml");
        fs::create_dir_all(&target_dir).unwrap();
        fs::create_dir_all(&parent_dir).unwrap();
        write(&temp.path().join("child.yaml"), "value\n");
        let resolver = SafeFileResolver::new(temp.path()).unwrap();

        let target_error = resolver
            .resolve(request("directory.yaml", None))
            .unwrap_err();
        assert!(matches!(
            problem(&target_error),
            ResolveProblem::TargetNotRegularFile { .. }
        ));

        let relative_parent = resolver
            .resolve(request("child.yaml", Some("relative-parent.yaml")))
            .unwrap_err();
        assert!(matches!(
            problem(&relative_parent),
            ResolveProblem::ParentIdNotAbsoluteCanonical { .. }
        ));

        let missing_parent = temp.path().join("missing-parent.yaml");
        let missing_parent = missing_parent.to_string_lossy();
        let missing_parent_error = resolver
            .resolve(request("child.yaml", Some(&missing_parent)))
            .unwrap_err();
        assert!(matches!(
            problem(&missing_parent_error),
            ResolveProblem::ParentResolveFailed { .. }
        ));

        let parent_dir = parent_dir.to_string_lossy();
        let parent_type_error = resolver
            .resolve(request("child.yaml", Some(&parent_dir)))
            .unwrap_err();
        assert!(matches!(
            problem(&parent_type_error),
            ResolveProblem::ParentNotRegularFile { .. }
        ));
    }

    #[test]
    fn filesystem_metadata_errors_are_propagated() {
        let temp = TempDir::new().unwrap();
        let resolver = SafeFileResolver::new(temp.path()).unwrap();
        let too_long = format!("{}.yaml", "a".repeat(300));

        let error = resolver.resolve(request(&too_long, None)).unwrap_err();
        assert!(
            !matches!(error, IncludeResolveError::FileInclude(_)),
            "an OS metadata error should be propagated directly: {error:?}"
        );
    }
}
