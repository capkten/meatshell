use super::*;

#[test]
fn marks_owner_and_preserves_source_tab() {
    let input = vec![
        ProcInfo {
            pid: 10,
            user: "alice".into(),
            priority: "20".into(),
            nice: "0".into(),
            virt: "100".into(),
            res: "50".into(),
            shr: "25".into(),
            state: "S".into(),
            cpu: 1.0,
            mem: 2.0,
            time: "00:00.01".into(),
            command: "own".into(),
        },
        ProcInfo {
            pid: 11,
            user: "root".into(),
            priority: "20".into(),
            nice: "0".into(),
            virt: "100".into(),
            res: "50".into(),
            shr: "25".into(),
            state: "S".into(),
            cpu: 3.0,
            mem: 4.0,
            time: "00:00.01".into(),
            command: "other".into(),
        },
    ];
    let rows = proc_rows(&input, "alice", "term-a");
    assert!(rows[0].own_process);
    assert!(!rows[1].own_process);
    assert!(rows.iter().all(|row| row.tab_id.as_str() == "term-a"));
}

#[test]
fn preserves_top_style_fields_in_process_rows() {
    let input = vec![ProcInfo {
        pid: 42,
        user: "root".into(),
        priority: "20".into(),
        nice: "0".into(),
        virt: "123456".into(),
        res: "45678".into(),
        shr: "7890".into(),
        state: "S".into(),
        cpu: 3.5,
        mem: 1.2,
        time: "00:01.23".into(),
        command: "java -jar demo.jar".into(),
    }];

    let row = &proc_rows(&input, "root", "term-a")[0];
    assert_eq!(row.priority, "20");
    assert_eq!(row.nice, "0");
    assert_eq!(row.virt, "123456");
    assert_eq!(row.res, "45678");
    assert_eq!(row.shr, "7890");
    assert_eq!(row.state, "S");
    assert_eq!(row.time, "00:01.23");
}

#[test]
fn privilege_rules_match_effective_login_user() {
    assert!(!process_needs_root("alice", "alice"));
    assert!(process_needs_root("alice", "root"));
    assert!(process_needs_root("alice", "bob"));
    assert!(!process_needs_root("root", "root"));
    assert!(!process_needs_root("root", "alice"));
}
