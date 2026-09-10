pub(crate) const DOCKER_COMPLETION_SETUP: &str = r#"if [ -z "${__ms_docker_completion_mode+x}" ]; then
    if [ -n "$BASH_VERSION" ]; then
        if complete -p docker >/dev/null 2>&1; then
            __ms_docker_completion_mode=native
        else
            __ms_docker_completion_mode=fallback
        fi
    elif [ -n "$ZSH_VERSION" ]; then
        if (( ${+_comps[docker]} )); then
            __ms_docker_completion_mode=native
        elif command -v compdef >/dev/null 2>&1 && (( ${+_comps} )); then
            __ms_docker_completion_mode=fallback
        else
            __ms_docker_completion_mode=unavailable
        fi
    else
        __ms_docker_completion_mode=unavailable
    fi
fi

if [ "$__ms_docker_completion_mode" = fallback ] && [ -z "${__ms_docker_completion_registered+x}" ]; then
    __ms_docker_completion_registered=1

    __ms_docker_option_takes_value() {
        case "$1" in
            --name|-e|--env|--env-file|-h|--hostname|-l|--label|--mount|--network|--publish|-p|--volume|-v|--workdir|-w|--user|-u|--restart|--platform|--entrypoint|--stop-timeout|--memory|--cpus|--pull|--signal|-s|--time|-t|--since|--tail|--until|--format|-f|--type|--detach-keys)
                return 0
                ;;
            *)
                return 1
                ;;
        esac
    }

    __ms_docker_images() {
        docker image ls --format "{{.Repository}}:{{.Tag}}" 2>/dev/null |
            while IFS= read -r image; do
                case "$image" in
                    *"<none>"*) ;;
                    *)
                        [ -n "$image" ] && printf "%s\n" "$image"
                        ;;
                esac
            done
    }

    __ms_docker_containers() {
        docker ps -a --format "{{.ID}}\t{{.Names}}" 2>/dev/null |
            while IFS="	" read -r container_id container_name; do
                [ -n "$container_id" ] && printf "%s\n" "$container_id"
                [ -n "$container_name" ] && printf "%s\n" "$container_name"
            done
    }

    __ms_docker_candidates() {
        case "$1" in
            image)
                __ms_docker_images
                ;;
            container)
                __ms_docker_containers
                ;;
            inspect)
                {
                    __ms_docker_images
                    __ms_docker_containers
                } | LC_ALL=C sort -u
                ;;
        esac
    }

    __ms_docker_bash_complete() {
        COMPREPLY=()
        local current="${COMP_WORDS[COMP_CWORD]}"
        case "$current" in
            -*)
                return 0
                ;;
        esac

        local subcommand=""
        local positional_count=0
        local expecting_value=0
        local end_options=0
        local token
        local i
        for ((i = 1; i < COMP_CWORD; i++)); do
            token="${COMP_WORDS[i]}"
            if [ "$expecting_value" -eq 1 ]; then
                expecting_value=0
                continue
            fi
            if [ "$end_options" -eq 0 ] && [ "$token" = "--" ]; then
                end_options=1
                continue
            fi
            if [ "$end_options" -eq 0 ] && __ms_docker_option_takes_value "$token"; then
                expecting_value=1
                continue
            fi
            if [ "$end_options" -eq 0 ] && [ "${token#-}" != "$token" ]; then
                return 0
            fi
            if [ -z "$subcommand" ]; then
                subcommand="$token"
            else
                positional_count=$((positional_count + 1))
            fi
        done
        [ "$expecting_value" -eq 0 ] || return 0

        local candidate_kind=""
        case "$subcommand" in
            run)
                [ "$positional_count" -eq 0 ] && candidate_kind=image
                ;;
            exec|start|stop|rm|logs)
                candidate_kind=container
                ;;
            inspect)
                candidate_kind=inspect
                ;;
            *)
                return 0
                ;;
        esac
        [ -n "$candidate_kind" ] || return 0

        local candidates
        candidates="$(__ms_docker_candidates "$candidate_kind")"
        COMPREPLY=( $(compgen -W "$candidates" -- "$current") )
    }

    _ms_docker_zsh_complete() {
        local current="${words[CURRENT]}"
        case "$current" in
            -*)
                return 0
                ;;
        esac

        local subcommand=""
        local positional_count=0
        local expecting_value=0
        local end_options=0
        local token
        local i
        for ((i = 2; i < CURRENT; i++)); do
            token="${words[i]}"
            if [ "$expecting_value" -eq 1 ]; then
                expecting_value=0
                continue
            fi
            if [ "$end_options" -eq 0 ] && [ "$token" = "--" ]; then
                end_options=1
                continue
            fi
            if [ "$end_options" -eq 0 ] && __ms_docker_option_takes_value "$token"; then
                expecting_value=1
                continue
            fi
            if [ "$end_options" -eq 0 ] && [ "${token#-}" != "$token" ]; then
                return 0
            fi
            if [ -z "$subcommand" ]; then
                subcommand="$token"
            else
                positional_count=$((positional_count + 1))
            fi
        done
        [ "$expecting_value" -eq 0 ] || return 0

        local candidate_kind=""
        case "$subcommand" in
            run)
                [ "$positional_count" -eq 0 ] && candidate_kind=image
                ;;
            exec|start|stop|rm|logs)
                candidate_kind=container
                ;;
            inspect)
                candidate_kind=inspect
                ;;
            *)
                return 0
                ;;
        esac
        [ -n "$candidate_kind" ] || return 0

        local candidates
        candidates="$(__ms_docker_candidates "$candidate_kind")"
        compadd -Q -- ${(f)candidates}
    }

    if [ -n "$BASH_VERSION" ]; then
        complete -F __ms_docker_bash_complete docker
    elif [ -n "$ZSH_VERSION" ]; then
        compdef _ms_docker_zsh_complete docker
    fi
fi"#;

#[cfg(test)]
mod tests {
    use super::DOCKER_COMPLETION_SETUP;

    #[test]
    fn setup_is_safe_to_embed_in_the_existing_single_quoted_eval() {
        assert!(!DOCKER_COMPLETION_SETUP.contains('\''));
        assert!(DOCKER_COMPLETION_SETUP.contains("__ms_docker_completion_mode"));
    }

    #[test]
    fn setup_has_no_boundary_newlines_for_prompt_composition() {
        assert!(!DOCKER_COMPLETION_SETUP.starts_with('\n'));
        assert!(!DOCKER_COMPLETION_SETUP.ends_with('\n'));
    }

    #[test]
    fn setup_has_one_latched_native_probe_and_one_fallback_registration_guard() {
        assert_eq!(
            DOCKER_COMPLETION_SETUP
                .matches("complete -p docker")
                .count(),
            1
        );
        assert!(DOCKER_COMPLETION_SETUP.contains("__ms_docker_completion_registered"));
        assert!(DOCKER_COMPLETION_SETUP.contains("__ms_docker_completion_mode=native"));
        assert!(DOCKER_COMPLETION_SETUP.contains("__ms_docker_completion_mode=fallback"));
    }

    #[test]
    fn zsh_native_probe_reads_comps_registry_without_compdef_query() {
        assert!(DOCKER_COMPLETION_SETUP.contains("${+_comps[docker]}"));
        assert!(!DOCKER_COMPLETION_SETUP.contains("compdef -p docker"));
    }

    #[test]
    fn setup_has_all_supported_command_branches_and_fixed_queries() {
        for command in ["run", "exec", "start", "stop", "rm", "logs", "inspect"] {
            assert!(
                DOCKER_COMPLETION_SETUP.contains(command),
                "missing completion branch for {command}"
            );
        }
        assert!(DOCKER_COMPLETION_SETUP.contains("docker image ls"));
        assert!(DOCKER_COMPLETION_SETUP.contains("docker ps -a"));
        assert!(DOCKER_COMPLETION_SETUP.contains("2>/dev/null"));
    }

    #[cfg(unix)]
    mod unix_shell_tests {
        use super::DOCKER_COMPLETION_SETUP;
        use std::ffi::OsString;
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;
        use std::process::{Command, Output};
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

        const FAKE_DOCKER: &str = r#"#!/bin/sh
if [ "${DOCKER_COMPLETION_FAIL:-0}" = 1 ]; then
    exit 23
elif [ "$1" = image ] && [ "$2" = ls ]; then
    printf '%s\n' 'nginx:latest' 'node:20'
elif [ "$1" = ps ] && [ "$2" = -a ]; then
    printf '%s\t%s\n' 'abc123' 'web' 'def456' 'worker'
fi
"#;

        struct FakeDocker {
            dir: PathBuf,
            path: OsString,
        }

        impl FakeDocker {
            fn new() -> Self {
                let suffix = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
                let dir = std::env::temp_dir().join(format!(
                    "meatshell-docker-completion-{}-{suffix}",
                    std::process::id()
                ));
                fs::create_dir(&dir).expect("create fake Docker directory");

                let docker = dir.join("docker");
                fs::write(&docker, FAKE_DOCKER).expect("write fake Docker executable");
                fs::set_permissions(&docker, fs::Permissions::from_mode(0o755))
                    .expect("make fake Docker executable");

                let mut path = OsString::from(&dir);
                if let Some(existing) = std::env::var_os("PATH") {
                    path.push(":");
                    path.push(existing);
                }

                Self { dir, path }
            }

            fn run(&self, script: &str, fail: bool) -> Output {
                Command::new("bash")
                    .arg("-c")
                    .arg(script)
                    .env("PATH", &self.path)
                    .env("DOCKER_COMPLETION_FAIL", if fail { "1" } else { "0" })
                    .env("DOCKER_COMPLETION_TEST_DIR", &self.dir)
                    .output()
                    .expect("start bash completion harness")
            }

            fn run_zsh(&self, script: &str, fail: bool) -> Output {
                Command::new("zsh")
                    .args(["-fc", script])
                    .env("PATH", &self.path)
                    .env("DOCKER_COMPLETION_FAIL", if fail { "1" } else { "0" })
                    .env("DOCKER_COMPLETION_TEST_DIR", &self.dir)
                    .output()
                    .expect("start zsh completion harness")
            }
        }

        impl Drop for FakeDocker {
            fn drop(&mut self) {
                fs::remove_dir_all(&self.dir).expect("remove fake Docker directory");
            }
        }

        fn shell_quote(value: &str) -> String {
            format!("'{}'", value.replace('\'', "'\\''"))
        }

        fn run_bash(script: &str, fail: bool) -> Output {
            let fixture = FakeDocker::new();
            fixture.run(script, fail)
        }

        fn run_zsh(script: &str, fail: bool) -> Output {
            let fixture = FakeDocker::new();
            fixture.run_zsh(script, fail)
        }

        fn bash_candidates(words: &[&str], current_index: usize) -> Vec<String> {
            let words = words
                .iter()
                .map(|word| shell_quote(word))
                .collect::<Vec<_>>()
                .join(" ");
            let script = format!(
                "eval {}\nCOMP_WORDS=({words})\nCOMP_CWORD={current_index}\n__ms_docker_bash_complete\nprintf '%s\\n' \"${{COMPREPLY[@]}}\"",
                shell_quote(DOCKER_COMPLETION_SETUP)
            );
            let output = run_bash(&script, false);
            assert!(
                output.status.success(),
                "bash completion failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::to_owned)
                .collect()
        }

        #[test]
        fn bash_run_completes_image_prefix() {
            assert_eq!(
                bash_candidates(&["docker", "run", "ng"], 2),
                vec!["nginx:latest".to_string()]
            );
        }

        #[test]
        fn bash_exec_completes_container_prefix() {
            assert_eq!(
                bash_candidates(&["docker", "exec", "we"], 2),
                vec!["web".to_string()]
            );
        }

        #[test]
        fn bash_inspect_merges_container_and_image_candidates() {
            assert_eq!(
                bash_candidates(&["docker", "inspect", ""], 2),
                vec![
                    "abc123".to_string(),
                    "def456".to_string(),
                    "nginx:latest".to_string(),
                    "node:20".to_string(),
                    "web".to_string(),
                    "worker".to_string(),
                ]
            );
        }

        #[test]
        fn bash_run_stops_image_completion_after_the_image_position() {
            assert!(bash_candidates(&["docker", "run", "nginx", "sh"], 3).is_empty());
        }

        #[test]
        fn native_completion_latches_without_replacing_or_reinstalling_it() {
            let script = format!(
                r#"
native_docker_complete() {{ COMPREPLY=(native); }}
complete -F native_docker_complete docker
eval {}
first_mode=$__ms_docker_completion_mode
first_registered=${{__ms_docker_completion_registered-<unset>}}
first_spec=$(builtin complete -p docker)
complete -r docker
eval {}
second_mode=$__ms_docker_completion_mode
second_registered=${{__ms_docker_completion_registered-<unset>}}
set +e
second_spec=$(builtin complete -p docker 2>&1)
second_status=$?
set -e
printf 'first_mode=%s\nfirst_registered=%s\nfirst_spec=%s\nsecond_mode=%s\nsecond_registered=%s\nsecond_spec=%s\nsecond_status=%s\n' \
    "$first_mode" "$first_registered" "$first_spec" "$second_mode" "$second_registered" "$second_spec" "$second_status"
"#,
                shell_quote(DOCKER_COMPLETION_SETUP),
                shell_quote(DOCKER_COMPLETION_SETUP)
            );
            let output = run_bash(&script, false);
            assert!(
                output.status.success(),
                "native latch harness failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                stdout.contains("first_mode=native"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.contains("first_registered=<unset>"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.contains("first_spec=complete -F native_docker_complete docker"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.contains("second_mode=native"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.contains("second_registered=<unset>"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.contains("second_status=1"),
                "captured output: {stdout}"
            );
        }

        #[test]
        fn fallback_completion_latches_without_repeating_native_probe() {
            let script = format!(
                r#"
probe_count=0
registration_count=0
complete() {{
    if [ "$1" = -p ] && [ "$2" = docker ]; then
        probe_count=$((probe_count + 1))
    fi
    if [ "$1" = -F ] && [ "$2" = __ms_docker_bash_complete ] && [ "$3" = docker ]; then
        registration_count=$((registration_count + 1))
    fi
    builtin complete "$@"
}}
builtin complete -r docker 2>/dev/null || true
eval {}
first_mode=$__ms_docker_completion_mode
first_registered=${{__ms_docker_completion_registered-<unset>}}
first_spec=$(builtin complete -p docker)
eval {}
second_mode=$__ms_docker_completion_mode
second_registered=${{__ms_docker_completion_registered-<unset>}}
second_spec=$(builtin complete -p docker)
printf 'probe_count=%s\nregistration_count=%s\nfirst_mode=%s\nfirst_registered=%s\nfirst_spec=%s\nsecond_mode=%s\nsecond_registered=%s\nsecond_spec=%s\n' \
    "$probe_count" "$registration_count" "$first_mode" "$first_registered" "$first_spec" "$second_mode" "$second_registered" "$second_spec"
"#,
                shell_quote(DOCKER_COMPLETION_SETUP),
                shell_quote(DOCKER_COMPLETION_SETUP)
            );
            let output = run_bash(&script, false);
            assert!(
                output.status.success(),
                "fallback latch harness failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                stdout.contains("probe_count=1"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.lines().any(|line| line == "registration_count=1"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.contains("first_mode=fallback"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.contains("first_registered=1"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.contains("first_spec=complete -F __ms_docker_bash_complete docker"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.contains("second_mode=fallback"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.contains("second_registered=1"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.contains("second_spec=complete -F __ms_docker_bash_complete docker"),
                "captured output: {stdout}"
            );
        }

        #[test]
        fn failed_docker_query_returns_no_candidates_or_shell_output() {
            let script = format!(
                r#"
eval {}
COMP_WORDS=(docker run ng)
COMP_CWORD=2
__ms_docker_bash_complete >"$DOCKER_COMPLETION_TEST_DIR/stdout" 2>"$DOCKER_COMPLETION_TEST_DIR/stderr"
status=$?
stdout_bytes=$(wc -c <"$DOCKER_COMPLETION_TEST_DIR/stdout" | tr -d '[:space:]')
stderr_bytes=$(wc -c <"$DOCKER_COMPLETION_TEST_DIR/stderr" | tr -d '[:space:]')
printf 'status=%s\nreply_count=%s\nstdout_bytes=%s\nstderr_bytes=%s\n' \
    "$status" "${{#COMPREPLY[@]}}" "$stdout_bytes" "$stderr_bytes"
"#,
                shell_quote(DOCKER_COMPLETION_SETUP)
            );
            let output = run_bash(&script, true);
            assert!(
                output.status.success(),
                "failed-query harness failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("status=0"), "captured output: {stdout}");
            assert!(
                stdout.contains("reply_count=0"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.lines().any(|line| line == "stdout_bytes=0"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.lines().any(|line| line == "stderr_bytes=0"),
                "captured output: {stdout}"
            );
        }

        #[test]
        fn zsh_smoke_completes_container_prefix_when_available() {
            let available = Command::new("zsh")
                .args(["-fc", "exit 0"])
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false);
            if !available {
                eprintln!("skipped zsh smoke test: zsh is unavailable");
                return;
            }

            let script = format!(
                r#"
typeset -A _comps
compdef() {{
    [ "$1" = -p ] && return 1
    return 0
}}
compadd() {{
    local candidate
    for candidate in "$@"; do
        case "$candidate" in
            -Q|--) ;;
            *) [[ "$candidate" == "$words[$CURRENT]"* ]] && print -r -- "$candidate" ;;
        esac
    done
}}
eval {}
words=(docker exec we)
CURRENT=3
_ms_docker_zsh_complete
"#,
                shell_quote(DOCKER_COMPLETION_SETUP)
            );
            let output = run_zsh(&script, false);
            assert!(
                output.status.success(),
                "zsh smoke harness failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .collect::<Vec<_>>(),
                vec!["web"]
            );
        }

        #[test]
        fn zsh_native_binding_is_detected_without_replacement_when_available() {
            let available = Command::new("zsh")
                .args(["-fc", "exit 0"])
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false);
            if !available {
                eprintln!("skipped zsh native-binding test: zsh is unavailable");
                return;
            }

            let script = format!(
                r#"
typeset -A _comps
native_docker_complete() {{ return 0 }}
_comps[docker]=native_docker_complete
compdef_calls=0
compdef() {{
    compdef_calls=$((compdef_calls + 1))
    [ "$1" = -p ] && return 1
    _comps[docker]="$1"
}}
eval {}
first_mode=$__ms_docker_completion_mode
first_binding=$_comps[docker]
unset '_comps[docker]'
eval {}
second_mode=$__ms_docker_completion_mode
binding_present=0
(( ${{+_comps[docker]}} )) && binding_present=1
registered_present=0
[ -n "${{__ms_docker_completion_registered+x}}" ] && registered_present=1
printf 'compdef_calls=%s\nfirst_mode=%s\nfirst_binding=%s\nsecond_mode=%s\nbinding_present=%s\nregistered_present=%s\n' \
    "$compdef_calls" "$first_mode" "$first_binding" "$second_mode" \
    "$binding_present" "$registered_present"
"#,
                shell_quote(DOCKER_COMPLETION_SETUP),
                shell_quote(DOCKER_COMPLETION_SETUP)
            );
            let output = run_zsh(&script, false);
            assert!(
                output.status.success(),
                "zsh native-binding harness failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                stdout.contains("compdef_calls=0"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.contains("first_mode=native"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.contains("first_binding=native_docker_complete"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.contains("second_mode=native"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.contains("binding_present=0"),
                "captured output: {stdout}"
            );
            assert!(
                stdout.contains("registered_present=0"),
                "captured output: {stdout}"
            );
        }
    }
}
