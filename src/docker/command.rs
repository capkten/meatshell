use super::{DockerExecResult, DockerRequest};
use std::io::ErrorKind;
use std::time::Duration;

pub(crate) fn docker_args(request: &DockerRequest) -> Vec<String> {
    match request {
        DockerRequest::Version => ["version", "--format", "{{json .}}"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        DockerRequest::Containers => ["ps", "-a", "--no-trunc", "--format", "{{json .}}"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        DockerRequest::Images => ["image", "ls", "--no-trunc", "--format", "{{json .}}"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        DockerRequest::InspectContainer(id) => {
            vec![
                "inspect".into(),
                "--type".into(),
                "container".into(),
                id.clone(),
            ]
        }
        DockerRequest::InspectImage(id) => vec!["image".into(), "inspect".into(), id.clone()],
    }
}

pub(crate) fn remote_command(request: &DockerRequest) -> String {
    let args = match request {
        DockerRequest::InspectContainer(id) => vec![
            "inspect".to_string(),
            "--type".to_string(),
            "container".to_string(),
            shell_quote(id),
        ],
        DockerRequest::InspectImage(id) => {
            vec!["image".to_string(), "inspect".to_string(), shell_quote(id)]
        }
        _ => docker_args(request)
            .into_iter()
            .map(remote_fixed_arg)
            .collect(),
    };
    format!("docker {}", args.join(" "))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn remote_fixed_arg(value: String) -> String {
    if value == "{{json .}}" {
        shell_quote(&value)
    } else {
        value
    }
}

pub(crate) async fn run_local(request: DockerRequest) -> DockerExecResult {
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::process::Command::new("docker")
            .args(docker_args(&request))
            .output(),
    )
    .await;

    match result {
        Err(_) => DockerExecResult {
            timed_out: true,
            ..Default::default()
        },
        Ok(Err(error)) if error.kind() == ErrorKind::NotFound => DockerExecResult {
            not_found: true,
            ..Default::default()
        },
        Ok(Err(error)) => DockerExecResult {
            stderr: error.to_string(),
            ..Default::default()
        },
        Ok(Ok(output)) => DockerExecResult {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code(),
            ..Default::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docker::{parse::classify_failure, DockerErrorKind, DockerRequest};

    #[test]
    fn list_commands_emit_json_lines_without_shell_pipeline() {
        assert_eq!(
            docker_args(&DockerRequest::Containers),
            vec!["ps", "-a", "--no-trunc", "--format", "{{json .}}"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn fixed_requests_emit_only_expected_docker_arguments() {
        assert_eq!(
            docker_args(&DockerRequest::Version),
            vec!["version", "--format", "{{json .}}"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            docker_args(&DockerRequest::Images),
            vec!["image", "ls", "--no-trunc", "--format", "{{json .}}"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            docker_args(&DockerRequest::InspectContainer("container-id".into())),
            vec!["inspect", "--type", "container", "container-id"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            docker_args(&DockerRequest::InspectImage("image-id".into())),
            vec!["image", "inspect", "image-id"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn inspect_id_is_quoted_in_remote_command() {
        let command = remote_command(&DockerRequest::InspectContainer("abc123".into()));
        assert_eq!(command, "docker inspect --type container 'abc123'");
    }

    #[test]
    fn inspect_image_id_is_quoted_without_quoting_fixed_arguments() {
        let command = remote_command(&DockerRequest::InspectImage("image; echo unsafe".into()));
        assert_eq!(command, "docker image inspect 'image; echo unsafe'");
    }

    #[test]
    fn inspect_id_with_apostrophe_remains_shell_safe() {
        let command = remote_command(&DockerRequest::InspectContainer("a'b".into()));
        assert_eq!(command, "docker inspect --type container 'a'\\''b'");
    }

    #[test]
    fn version_remote_command_keeps_json_template_as_one_argument() {
        assert_eq!(
            remote_command(&DockerRequest::Version),
            "docker version --format '{{json .}}'"
        );
    }

    #[test]
    fn containers_remote_command_keeps_json_template_as_one_argument() {
        assert_eq!(
            remote_command(&DockerRequest::Containers),
            "docker ps -a --no-trunc --format '{{json .}}'"
        );
    }

    #[test]
    fn images_remote_command_keeps_json_template_as_one_argument() {
        assert_eq!(
            remote_command(&DockerRequest::Images),
            "docker image ls --no-trunc --format '{{json .}}'"
        );
    }

    #[test]
    fn not_found_result_is_classified_as_not_installed() {
        let result = DockerExecResult {
            not_found: true,
            ..Default::default()
        };
        assert_eq!(classify_failure(&result), DockerErrorKind::NotInstalled);
    }
}
