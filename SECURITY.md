serde-saphyr supports only the latest released version. Security fixes are provided via new releases; we do not backport fixes to older versions.

If you believe you've found a security vulnerability, please report it privately to the [principal maintainer](https://github.com/bourumir-wyngs). If you're unsure whether an issue is security-relevant, report it privately anyway.

Examples of security-relevant issues (**also in dependencies**) include:
- memory safety issues (including [miri](https://github.com/rust-lang/miri) findings)
- arbitrary code execution
- data integrity/validation bypass (e.g., parsing/serialization producing incorrect results in a way that can be exploited)
- denial-of-service when processing untrusted input (panic or hang)

If a vulnerability is confirmed, we will release a patched version and may yank affected versions on crates.io to reduce accidental adoption. Note that yanking does not 
remove already-downloaded artifacts and does not update existing Cargo.lock files, so upgrading to the fixed release is still required.

For non-security bugs and general reliability issues that do not have a security impact, please open a GitHub issue.
