use ainline::{
    config::Kind,
    provider::{self, Provider, ProviderError},
};
use std::path::Path;

#[test]
fn new_provider_names_deserialize_and_dispatch() {
    for name in ["copilot", "antigravity"] {
        let kind: Kind = serde_json::from_str(&format!("\"{name}\"")).unwrap();
        assert_eq!(serde_json::to_string(&kind).unwrap(), format!("\"{name}\""));
        let command = provider::adapter(kind).command(Path::new("custom-binary"), "custom-model");
        assert_eq!(command.get_program(), "custom-binary");
        assert!(command.get_args().any(|arg| arg == "custom-model"));
    }
}

#[test]
fn copilot_preserves_piped_context_and_disables_tools() {
    let p = provider::copilot::Copilot;
    let command = p.command(Path::new("copilot"), "model");
    let args: Vec<_> = command
        .get_args()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    assert!(
        !args
            .iter()
            .any(|s| s == "-p" || s == "--prompt" || s == "--allow-all")
    );
    assert_eq!(args.last().unwrap(), "--available-tools");
    assert!(args.iter().any(|s| s == "--silent"));
    assert!(args.iter().any(|s| s == "--no-ask-user"));
    let prompt = "Review:\n'quoted' $(literal) --help ñ";
    assert_eq!(p.input("model", prompt), prompt);
    assert_eq!(p.output(b"response\n".to_vec()).unwrap(), b"response\n");
}

#[test]
fn antigravity_encodes_exactly_one_user_event() {
    let p = provider::antigravity::Antigravity;
    let prompt = "Review:\n\"quoted\" $(literal) ñ\n";
    let input = p.input("model", prompt);
    assert_eq!(input.lines().count(), 1);
    assert!(input.ends_with('\n'));
    let event: serde_json::Value = serde_json::from_str(&input).unwrap();
    assert_eq!(event["event"], "user");
    assert_eq!(event["message"]["content"], prompt);
    let command = p.command(Path::new("agy"), "model");
    let args: Vec<_> = command
        .get_args()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    assert!(args.windows(2).any(|a| a == ["--mode", "plan"]));
    assert!(
        args.windows(2)
            .any(|a| a == ["--input-format", "stream-json"])
    );
    assert!(
        args.windows(2)
            .any(|a| a == ["--output-format", "stream-json"])
    );
    assert!(
        !args
            .iter()
            .any(|s| s == "--dangerously-skip-permissions" || s == "-p")
    );
}

#[test]
fn antigravity_extracts_final_text_without_duplicate_deltas() {
    let data = concat!(
        "{\"event\":\"init\"}\n",
        "{\"event\":\"step_update\",\"step_update\":{\"text_delta\":\"partial\"}}\n",
        "{\"event\":\"future_event\"}\n",
        "{\"event\":\"result\",\"result\":{\"status\":\"SUCCESS\",\"response\":\"complete\\n\"}}\n"
    );
    assert_eq!(
        provider::antigravity::Antigravity
            .output(data.as_bytes().to_vec())
            .unwrap(),
        b"complete\n"
    );
}

#[test]
fn antigravity_rejects_errors_incomplete_or_malformed_output() {
    let p = provider::antigravity::Antigravity;
    for data in [
        "",
        "not json",
        "{\"event\":\"init\"}",
        r#"{"event":"result","result":{"status":"ERROR","error":"quota exceeded"}}"#,
        r#"{"event":"result","result":{"status":"WAITING"}}"#,
        r#"{"event":"result","result":{"status":"SUCCESS","response":""}}"#,
        r#"{"event":"result","result":{"status":"SUCCESS"}}"#,
    ] {
        assert!(
            matches!(
                p.output(data.as_bytes().to_vec()),
                Err(ProviderError::Response(_))
            ),
            "{data}"
        );
    }
    let success =
        "{\"event\":\"result\",\"result\":{\"status\":\"SUCCESS\",\"response\":\"ok\"}}\n";
    assert!(p.output(success.repeat(2).into_bytes()).is_err());
}

#[test]
fn antigravity_cancellation_is_not_a_retryable_failure() {
    for status in ["CANCELED", "INTERRUPTED"] {
        let data = serde_json::json!({"event":"result","result":{"status":status}}).to_string();
        assert!(matches!(
            provider::antigravity::Antigravity.output(data.into_bytes()),
            Err(ProviderError::Interrupted)
        ));
    }
}

#[cfg(unix)]
#[test]
fn antigravity_generic_exit_preserves_structured_cancellation() {
    use ainline::config::Model;
    use std::{fs, os::unix::fs::PermissionsExt};
    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let fixture =
        Fixture(std::env::temp_dir().join(format!("ainline-provider-test-{}", std::process::id())));
    fs::create_dir(&fixture.0).unwrap();
    let binary = fixture.0.join("agy-mock");
    fs::write(&binary, "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{\"event\":\"result\",\"result\":{\"status\":\"CANCELED\"}}'\nexit 1\n").unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    let model = Model {
        provider: Kind::Antigravity,
        model: "test-model".into(),
        binary: binary.to_string_lossy().into_owned(),
    };
    assert!(matches!(
        provider::execute(&model, &binary, "hello\nworld", &fixture.0),
        Err(ProviderError::Interrupted)
    ));
}

#[test]
fn passthrough_rejects_protocol_overrides() {
    for flag in [
        "--prompt=ignored",
        "-p",
        "--output-format=json",
        "--json",
        "--output-last-message=answer.txt",
        "-oanswer.txt",
        "--input-format",
        "--model=wrong",
        "--continue",
        "-pREPLACEMENT",
        "-msonnet",
        "-spREPLACEMENT",
    ] {
        assert!(provider::validate_args(&[flag.into()]).is_err(), "{flag}");
    }
    provider::validate_args(&["--effort".into(), "high".into()]).unwrap();
}

#[test]
fn claude_stream_decodes_split_utf8_and_avoids_duplicate_results() {
    use ainline::provider::claude::StreamDecoder;
    let source = concat!(
        "{\"type\":\"system\"}\n",
        "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"héllo\"}}}\n",
        "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"héllo\"}]}}\n",
        "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"héllo\"}\n"
    );
    let mut decoder = StreamDecoder::default();
    let mut text = Vec::new();
    for byte in source.as_bytes() {
        for chunk in decoder.push(&[*byte]).unwrap() {
            text.extend(chunk);
        }
    }
    decoder.finish().unwrap();
    assert_eq!(String::from_utf8(text).unwrap(), "héllo");
    let mut decoder = StreamDecoder::default();
    assert_eq!(
        decoder
            .push(b"{\"type\":\"result\",\"is_error\":false,\"result\":\"fallback\"}\n")
            .unwrap()
            .concat(),
        b"fallback"
    );
    decoder.finish().unwrap();
    let mut decoder = StreamDecoder::default();
    decoder
        .push(b"{\"type\":\"result\",\"is_error\":true,\"result\":\"authentication failed\"}\n")
        .unwrap();
    assert!(
        decoder
            .finish()
            .unwrap_err()
            .to_string()
            .contains("authentication failed")
    );
    assert!(StreamDecoder::default().finish().is_err());
    assert!(StreamDecoder::default().push(b"not-json\n").is_err());
}
