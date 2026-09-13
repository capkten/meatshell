use super::super::*;

#[test]
fn paste_normalizes_newlines_to_cr() {
    assert_eq!(
        normalize_pasted_newlines("sudo apt install \\\r\n  docker-ce"),
        "sudo apt install \\\r  docker-ce"
    );
    assert_eq!(normalize_pasted_newlines("a\nb\nc"), "a\rb\rc");
    assert_eq!(normalize_pasted_newlines("a\rb"), "a\rb");
    assert_eq!(normalize_pasted_newlines("echo hi"), "echo hi");
}

#[test]
fn command_bar_preserves_multiline_heredoc() {
    let command = "cat <<'EOF'\nHEREDOC-1\n中文-HEREDOC-2\nEOF\n";
    let (history, bytes) = encode_command_bar_input(command);
    assert_eq!(history.as_deref(), Some(command.trim_end()));
    assert_eq!(bytes, command.as_bytes());
    assert!(!history.unwrap().lines().any(|line| line.starts_with(' ')));
}

#[test]
fn empty_command_bar_submission_sends_enter_without_history() {
    for input in ["", "   ", "\t"] {
        let (history, bytes) = encode_command_bar_input(input);
        assert_eq!(history, None);
        assert_eq!(bytes, b"\n");
    }
}

#[test]
fn paste_uses_remote_bracketed_paste_mode() {
    assert_eq!(
        encode_pasted_text("first\r\n  second", true),
        b"\x1b[200~first\r  second\x1b[201~"
    );
    assert_eq!(
        encode_pasted_text("safe\x1b[201~\x03text", true),
        b"\x1b[200~safe[201~text\x1b[201~"
    );
    assert_eq!(
        encode_pasted_text("first\r\nsecond", false),
        b"first\rsecond"
    );
}

#[test]
fn long_pastes_switch_to_large_review() {
    assert!(!paste_requires_large_review("short prompt\nsecond line"));
    assert!(!paste_requires_large_review(&"a".repeat(600)));
    assert!(paste_requires_large_review(&"a".repeat(601)));
    assert!(!paste_requires_large_review(&["line"; 12].join("\r\n")));
    assert!(paste_requires_large_review(&vec!["line"; 13].join("\r\n")));
}

#[test]
fn paste_preview_is_bounded_before_slint_layout() {
    let text = format!("{}\nTAIL", "x".repeat(3_000));
    let preview = paste_preview(&text);

    assert!(!preview.truncated);
    assert_eq!(preview.text, text);
}

#[test]
fn paste_preview_keeps_text_through_one_mib() {
    let text = "x".repeat(1_048_576);
    let preview = paste_preview(&text);

    assert_eq!(preview.text.replace('\n', ""), text);
    assert!(!preview.truncated);
}

#[test]
fn paste_preview_segments_long_unbroken_text_for_slint_layout() {
    let text = "x".repeat(1_048_576);
    let preview = paste_preview(&text);

    assert!(preview.text.contains('\n'));
    assert_eq!(preview.text.replace('\n', ""), text);
    assert!(preview
        .text
        .lines()
        .all(|line| line.chars().count() <= 4_096));
}

#[test]
fn paste_preview_is_split_into_bounded_render_chunks() {
    let text = "x".repeat(1_048_576);
    let preview = paste_preview(&text);

    assert!(preview.chunks.len() > 1);
    assert_eq!(preview.chunks.concat(), preview.text);
    assert!(preview
        .chunks
        .iter()
        .all(|chunk| chunk.chars().count() <= 4_096));
}

#[test]
fn paste_preview_truncates_after_one_mib_and_reports_it() {
    let text = format!("{}TAIL", "x".repeat(1_048_576));
    let preview = paste_preview(&text);

    assert!(preview.truncated);
    assert!(preview.text.ends_with("\n…"));
    assert!(preview.text.replace('\n', "").len() <= 1_048_576);
    assert!(!preview.text.contains("TAIL"));
}

#[test]
fn paste_preview_keeps_multibyte_text_valid_at_the_boundary() {
    let text = format!("{}TAIL", "界".repeat(400_000));
    let preview = paste_preview(&text);

    assert!(preview.truncated);
    assert!(preview.text.ends_with("\n…"));
    assert!(preview.text.replace('\n', "").len() <= 1_048_576);
    assert!(!preview.text.contains("TAIL"));
    let prefix = preview.text.strip_suffix("\n…").unwrap();
    assert!(prefix.ends_with('界'));
}

#[test]
fn very_large_single_line_pastes_still_require_review() {
    assert!(paste_requires_review(&"x".repeat(100 * 1024 + 1), false));
    assert!(!paste_requires_review(&"x".repeat(100 * 1024), false));
    assert!(paste_requires_review("first\nsecond", true));
    assert!(!paste_requires_review("first\nsecond", false));
}

#[test]
fn confirmed_paste_is_enqueued_before_following_key() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    enqueue_confirmed_paste(&tx, PreparedPaste::new("abc"), false);
    tx.send(SessionCommand::RawInput(b"\r".to_vec())).unwrap();

    match rx.try_recv().unwrap() {
        SessionCommand::RawInput(bytes) => assert_eq!(bytes, b"abc"),
        _ => panic!("expected paste input"),
    }
    match rx.try_recv().unwrap() {
        SessionCommand::RawInput(bytes) => assert_eq!(bytes, b"\r"),
        _ => panic!("expected following key"),
    }
}

#[test]
fn prepared_paste_keeps_both_encoding_modes() {
    let text = "a\r\nb\x1b\x03";
    let prepared = PreparedPaste::new(text);

    assert_eq!(
        prepared.clone().into_bytes(false),
        encode_pasted_text(text, false)
    );
    assert_eq!(prepared.into_bytes(true), encode_pasted_text(text, true));
}
