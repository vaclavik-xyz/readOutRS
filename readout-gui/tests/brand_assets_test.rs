use std::path::Path;

#[test]
fn discoverable_project_icon_matches_master() {
    // assets/icon.png exists only so tools such as T3 Code find the project icon;
    // docs/icon_rounded.png is the master (also shown in README.md).
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let master = std::fs::read(root.join("docs/icon_rounded.png")).expect("read master icon");
    let copy = std::fs::read(root.join("assets/icon.png")).expect("read assets/icon.png");
    assert!(
        copy == master,
        "copy docs/icon_rounded.png to assets/icon.png"
    );
}
