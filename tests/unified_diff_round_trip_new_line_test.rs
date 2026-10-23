mod common;

use common::{applied, java_split};
use diffutils::read_unified_diff;

#[test]
#[ignore = "disabled in the Java suite (for next release)"]
fn test_issue135_missing_no_new_line_in_patched() {
    let before = "rootProject.name = \"sample-repo\"";
    let after = "rootProject.name = \"sample-repo\"\n";
    let patch = "diff --git a/settings.gradle b/settings.gradle\nindex ef3b8e2..ab30124 100644\n--- a/settings.gradle\n+++ b/settings.gradle\n@@ -1 +1 @@\n-rootProject.name = \"sample-repo\"\n\\ No newline at end of file\n+rootProject.name = \"sample-repo\"\n";
    let diff = read_unified_diff(patch).expect("a well-formed diff");
    let patched = applied(&diff.files[0].patch, &java_split(before, "\n")).expect("the patch applies");
    assert_eq!(patched.join("\n"), after);
}
