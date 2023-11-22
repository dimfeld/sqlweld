use tempfile::TempDir;

use super::{build, Error, Options};

const UPDATE_SOME_OBJECTS: &'static str =
    include_str!("../test_data/update_some_objects.sql.liquid");
const GET_SOME_OBJECTS: &'static str = include_str!("../test_data/get_some_objects.sql.liquid");
const OTHER_TEMPLATE: &'static str = include_str!("../test_data/other_template.liquid");
const PERM_CHECK: &'static str = include_str!("../test_data/perm_check.partial.sql.liquid");

const EXPECTED_UPDATE_SOME_OBJECTS: &'static str =
    include_str!("../test_data/update_some_objects.sql");
const EXPECTED_GET_SOME_OBJECTS: &'static str = include_str!("../test_data/get_some_objects.sql");
const HEADER: &'static str = "-- Autogenerated by sqlweld";

fn strip_header(s: &str) -> &str {
    s.strip_prefix(HEADER).unwrap_or(s).trim_start()
}

fn apply_header(header: &str, base_expected: &str) -> String {
    let expected = strip_header(base_expected);
    if header.is_empty() {
        expected.to_string()
    } else {
        format!("{header}\n\n{expected}")
    }
}

fn create_input() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let dirpath = dir.path();

    std::fs::write(
        dirpath.join("update_some_objects.sql.liquid"),
        UPDATE_SOME_OBJECTS,
    )
    .unwrap();
    std::fs::write(
        dirpath.join("get_some_objects.sql.liquid"),
        GET_SOME_OBJECTS,
    )
    .unwrap();
    std::fs::write(dirpath.join("other_template.liquid"), OTHER_TEMPLATE).unwrap();
    std::fs::write(dirpath.join("perm_check.partial.sql.liquid"), PERM_CHECK).unwrap();

    dir
}

#[test]
fn normal_test() {
    let dir = create_input();
    let path = dir.path().to_owned();

    build(Options {
        input: Some(path.clone()),
        ..Default::default()
    })
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(path.join("update_some_objects.sql")).unwrap(),
        apply_header(HEADER, EXPECTED_UPDATE_SOME_OBJECTS)
    );
    assert_eq!(
        std::fs::read_to_string(path.join("get_some_objects.sql")).unwrap(),
        apply_header(HEADER, EXPECTED_GET_SOME_OBJECTS)
    );

    assert!(std::fs::File::open(path.join("other_template.sql")).is_err());
    assert!(std::fs::File::open(path.join("perm_check.sql")).is_err());
}

#[test]
fn separate_output() {
    let dir = create_input();
    let path = dir.path().to_owned();

    let output = dir.path().join("output");
    std::fs::create_dir(&output).unwrap();

    build(Options {
        input: Some(path.clone()),
        output: Some(output.clone()),
        ..Default::default()
    })
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(output.join("update_some_objects.sql")).unwrap(),
        apply_header(HEADER, EXPECTED_UPDATE_SOME_OBJECTS)
    );
    assert_eq!(
        std::fs::read_to_string(output.join("get_some_objects.sql")).unwrap(),
        apply_header(HEADER, EXPECTED_GET_SOME_OBJECTS)
    );

    assert!(std::fs::File::open(output.join("other_template.sql")).is_err());
    assert!(std::fs::File::open(output.join("perm_check.sql")).is_err());
}

#[test]
fn custom_header() {
    let dir = create_input();
    let path = dir.path().to_owned();

    build(Options {
        input: Some(path.clone()),
        header: Some("custom header".to_string()),
        ..Default::default()
    })
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(path.join("update_some_objects.sql")).unwrap(),
        apply_header("-- custom header", EXPECTED_UPDATE_SOME_OBJECTS)
    );
    assert_eq!(
        std::fs::read_to_string(path.join("get_some_objects.sql")).unwrap(),
        apply_header("-- custom header", EXPECTED_GET_SOME_OBJECTS)
    );

    assert!(std::fs::File::open(path.join("other_template.sql")).is_err());
    assert!(std::fs::File::open(path.join("perm_check.sql")).is_err());
}

#[test]
fn no_header() {
    let dir = create_input();
    let path = dir.path().to_owned();

    build(Options {
        input: Some(path.clone()),
        header: Some("".to_string()),
        ..Default::default()
    })
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(path.join("update_some_objects.sql")).unwrap(),
        strip_header(EXPECTED_UPDATE_SOME_OBJECTS)
    );
    assert_eq!(
        std::fs::read_to_string(path.join("get_some_objects.sql")).unwrap(),
        strip_header(EXPECTED_GET_SOME_OBJECTS)
    );

    assert!(std::fs::File::open(path.join("other_template.sql")).is_err());
    assert!(std::fs::File::open(path.join("perm_check.sql")).is_err());
}

#[test]
fn custom_header_multiline() {
    let dir = create_input();
    let path = dir.path().to_owned();

    build(Options {
        input: Some(path.clone()),
        header: Some("custom header\r\nand another line".to_string()),
        ..Default::default()
    })
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(path.join("update_some_objects.sql")).unwrap(),
        apply_header(
            "-- custom header\n-- and another line",
            EXPECTED_UPDATE_SOME_OBJECTS
        )
    );
    assert_eq!(
        std::fs::read_to_string(path.join("get_some_objects.sql")).unwrap(),
        apply_header(
            "-- custom header\n-- and another line",
            EXPECTED_GET_SOME_OBJECTS
        )
    );

    assert!(std::fs::File::open(path.join("other_template.sql")).is_err());
    assert!(std::fs::File::open(path.join("perm_check.sql")).is_err());
}

#[test]
fn custom_extension() {
    let dir = create_input();
    let path = dir.path().to_owned();

    build(Options {
        input: Some(path.clone()),
        extension: Some("gen.sql".to_string()),
        ..Default::default()
    })
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(path.join("update_some_objects.gen.sql")).unwrap(),
        apply_header(HEADER, EXPECTED_UPDATE_SOME_OBJECTS)
    );
    assert_eq!(
        std::fs::read_to_string(path.join("get_some_objects.gen.sql")).unwrap(),
        apply_header(HEADER, EXPECTED_GET_SOME_OBJECTS)
    );

    assert!(std::fs::File::open(path.join("other_template.sql")).is_err());
    assert!(std::fs::File::open(path.join("perm_check.sql")).is_err());
}

#[test]
fn duplicate_partials() {
    let dir = create_input();
    let path = dir.path().to_owned();

    let dir1 = path.join("dir1");
    std::fs::create_dir(&dir1).unwrap();
    let dir2 = path.join("dir2");
    std::fs::create_dir(&dir2).unwrap();

    std::fs::write(dir1.join("dup.partial.sql.liquid"), "abc").unwrap();
    std::fs::write(dir2.join("dup.partial.sql.liquid"), "def").unwrap();

    let err = build(Options {
        input: Some(path.clone()),
        ..Default::default()
    })
    .expect_err("should fail");

    assert!(matches!(err.current_context(), Error::DuplicatePartial));
}
