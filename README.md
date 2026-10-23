# diffutils

## Status

[![Rust CI](https://github.com/java-diff-utils/java-diff-utils/workflows/Rust%20CI/badge.svg)](https://github.com/java-diff-utils/java-diff-utils/actions?query=workflow%3A%22Rust+CI%22)

## Intro

Diff Utils library is an open source library for performing comparison operations between texts: computing diffs, applying patches, generating unified diffs or parsing them, generating diff output for easy future displaying (like side-by-side view) and so on.

The main reason to build this library was the lack of easy-to-use libraries with all the usual stuff you need while working with diff files. Originally it was inspired by JRCS library and its nice design of diff module.

**This is originally a fork of java-diff-utils from Google Code Archive.** This version is a Rust port of java-diff-utils 4.18 that keeps the results of the Java library.

## GPG Signature Validation

The GPG signing key in [KEYS] is used for this project's artifacts.

## Examples

**Producing a one liner including all difference information.**

```rust
use diffutils::DiffRowGenerator;

let generator = DiffRowGenerator::builder()
    .show_inline_diffs(true)
    .merge_original_revised(true)
    .inline_diff_by_word(true)
    .old_tag(|_, _| "~".to_string())
    .new_tag(|_, _| "**".to_string())
    .build();

let rows = generator.generate_diff_rows(
    &["This is a test sentence.".to_string()],
    &["This is a test for diffutils.".to_string()],
);
println!("{}", rows[0].old_line);
```

This is a test ~sentence~**for diffutils**.

**Producing a side-by-side view of computed differences.**

```rust
use diffutils::DiffRowGenerator;

let generator = DiffRowGenerator::builder()
    .show_inline_diffs(true)
    .inline_diff_by_word(true)
    .old_tag(|_, _| "~".to_string())
    .new_tag(|_, _| "**".to_string())
    .build();
let original: Vec<String> = ["This is a test sentence.", "This is the second line.", "And here is the finish."]
    .iter().map(|s| s.to_string()).collect();
let revised: Vec<String> = ["This is a test for diffutils.", "This is the second line."]
    .iter().map(|s| s.to_string()).collect();

println!("|original|new|");
println!("|--------|---|");
for row in generator.generate_diff_rows(&original, &revised) {
    println!("|{}|{}|", row.old_line, row.new_line);
}
```

|original|new|
|--------|---|
|This is a test ~sentence~.|This is a test **for diffutils**.|
|This is the second line.|This is the second line.|
|~And here is the finish.~||

**Computing, printing, parsing and applying a patch.**

```rust
use diffutils::{diff, generate_unified_diff, parse_unified_diff};

let original: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
let revised: Vec<String> = vec!["a".into(), "B".into(), "c".into()];
let patch = diff(&original, &revised);
let text = generate_unified_diff(Some("a.txt"), Some("b.txt"), &original, &patch, 1);
let mut target = original.clone();
parse_unified_diff(&text).apply_to(&mut target).expect("patch applies");
assert_eq!(target, revised);
```

## Main Features

* Computing the difference between two texts.
* Patch and unpatch the text with the given patch, strictly, with fuzz or with merge-conflict markers
* Parsing and writing the unified diff format, including multi-file git diffs
* Producing human-readable differences
* Inline difference construction
* Algorithms:
  * Myers standard algorithm
  * Myers with linear space improvement

## To Install

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
diffutils = { path = "../diffutils" }
```

Build and test with `cargo build --release` and `cargo test`.
