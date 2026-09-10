pub(crate) const DOCKER_COMPLETION_SETUP: &str = r#"if [ -z "${__ms_docker_completion_mode+x}" ]; then
    if [ -n "$BASH_VERSION" ]; then
        if complete -p docker >/dev/null 2>&1; then
            __ms_docker_completion_mode=native
        else
            __ms_docker_completion_mode=fallback
        fi
    elif [ -n "$ZSH_VERSION" ]; then
        if command -v compdef >/dev/null 2>&1 && compdef -p docker >/dev/null 2>&1; then
            __ms_docker_completion_mode=native
        elif command -v compdef >/dev/null 2>&1; then
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
}
