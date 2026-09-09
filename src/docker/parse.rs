use super::{
    ContainerFilter, ContainerState, DockerContainerDetail, DockerContainerSummary, DockerError,
    DockerErrorKind, DockerExecResult, DockerImageDetail, DockerImageSummary,
};
use serde_json::Value;

pub(crate) fn parse_container_rows(
    stdout: &str,
) -> Result<Vec<DockerContainerSummary>, DockerError> {
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let value = parse_object(line)?;
            Ok(DockerContainerSummary {
                id: string_field(&value, "ID"),
                name: normalize_name(&string_field(&value, "Names")),
                image: string_field(&value, "Image"),
                state: if string_field(&value, "State").eq_ignore_ascii_case("running") {
                    ContainerState::Running
                } else {
                    ContainerState::Stopped
                },
                status: string_field(&value, "Status"),
                created: string_field(&value, "CreatedAt"),
                ports: string_field(&value, "Ports"),
            })
        })
        .collect()
}

pub(crate) fn parse_image_rows(stdout: &str) -> Result<Vec<DockerImageSummary>, DockerError> {
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let value = parse_object(line)?;
            Ok(DockerImageSummary {
                id: string_field(&value, "ID"),
                repository: string_field(&value, "Repository"),
                tag: string_field(&value, "Tag"),
                size: string_field(&value, "Size"),
                created: string_field(&value, "CreatedAt"),
            })
        })
        .collect()
}

pub(crate) fn parse_container_detail(stdout: &str) -> Result<DockerContainerDetail, DockerError> {
    let value = first_inspect_value(stdout)?;
    Ok(DockerContainerDetail {
        id: string_field(&value, "Id"),
        image: nested_string(&value, &["Config", "Image"]),
        command: value_to_text(value.pointer("/Config/Cmd")),
        created: string_field(&value, "Created"),
        ports: value_to_text(value.pointer("/NetworkSettings/Ports")),
        mounts: value_to_text(value.pointer("/Mounts")),
        networks: value_to_text(value.pointer("/NetworkSettings/Networks")),
    })
}

pub(crate) fn parse_image_detail(stdout: &str) -> Result<DockerImageDetail, DockerError> {
    let value = first_inspect_value(stdout)?;
    Ok(DockerImageDetail {
        id: string_field(&value, "Id"),
        repository_tags: value_to_text(value.get("RepoTags")),
        size: value_to_text(value.get("Size")),
        created: string_field(&value, "Created"),
    })
}

pub(crate) fn filter_containers(
    rows: &[DockerContainerSummary],
    query: &str,
    filter: ContainerFilter,
) -> Vec<DockerContainerSummary> {
    let query = query.to_lowercase();
    rows.iter()
        .filter(|row| match filter {
            ContainerFilter::All => true,
            ContainerFilter::Running => row.state == ContainerState::Running,
            ContainerFilter::Stopped => row.state == ContainerState::Stopped,
        })
        .filter(|row| {
            query.is_empty()
                || [row.name.as_str(), row.id.as_str(), row.image.as_str()]
                    .iter()
                    .any(|field| field.to_lowercase().contains(&query))
        })
        .cloned()
        .collect()
}

pub(crate) fn filter_images(rows: &[DockerImageSummary], query: &str) -> Vec<DockerImageSummary> {
    let query = query.to_lowercase();
    rows.iter()
        .filter(|row| {
            query.is_empty()
                || [row.repository.as_str(), row.tag.as_str(), row.id.as_str()]
                    .iter()
                    .any(|field| field.to_lowercase().contains(&query))
        })
        .cloned()
        .collect()
}

pub(crate) fn classify_failure(result: &DockerExecResult) -> DockerErrorKind {
    if result.not_found {
        return DockerErrorKind::NotInstalled;
    }
    let message = format!("{} {}", result.stderr, result.stdout).to_lowercase();
    if message.contains("command not found") || message.contains("not recognized as an internal") {
        DockerErrorKind::NotInstalled
    } else if message.contains("permission denied") || message.contains("access denied") {
        DockerErrorKind::PermissionDenied
    } else if message.contains("cannot connect to the docker daemon")
        || message.contains("is the docker daemon running")
        || message.contains("docker daemon") && message.contains("connect")
    {
        DockerErrorKind::DaemonUnavailable
    } else {
        DockerErrorKind::CommandFailed
    }
}

fn parse_object(line: &str) -> Result<Value, DockerError> {
    let value: Value =
        serde_json::from_str(line).map_err(|error| parse_error(error.to_string()))?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(parse_error("expected a JSON object".to_string()))
    }
}

fn first_inspect_value(stdout: &str) -> Result<Value, DockerError> {
    let value: Value =
        serde_json::from_str(stdout).map_err(|error| parse_error(error.to_string()))?;
    match value {
        Value::Array(mut values) => values
            .drain(..)
            .find(Value::is_object)
            .ok_or_else(|| parse_error("expected a non-empty inspect array".to_string())),
        Value::Object(value) => Ok(Value::Object(value)),
        _ => Err(parse_error(
            "expected an inspect object or array".to_string(),
        )),
    }
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn nested_string(value: &Value, path: &[&str]) -> String {
    let mut current = value;
    for key in path {
        current = match current.get(*key) {
            Some(value) => value,
            None => return String::new(),
        };
    }
    current.as_str().unwrap_or_default().to_string()
}

fn normalize_name(name: &str) -> String {
    name.strip_prefix('/').unwrap_or(name).to_string()
}

fn value_to_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(value) if !value.is_null() => serde_json::to_string(value).unwrap_or_default(),
        _ => String::new(),
    }
}

fn parse_error(message: String) -> DockerError {
    DockerError {
        kind: DockerErrorKind::ParseFailed,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_lines_and_keeps_stopped_containers() {
        let stdout = r#"{"ID":"abc123","Image":"nginx:1.27","Names":"web","State":"running","Status":"Up 3 days","CreatedAt":"2026-09-08 10:00:00 +0000 UTC","Ports":"0.0.0.0:80->80/tcp"}
{"ID":"def456","Image":"worker:old","Names":"worker","State":"exited","Status":"Exited (0) 2 days ago","CreatedAt":"2026-09-06 10:00:00 +0000 UTC","Ports":""}"#;
        let rows = parse_container_rows(stdout).expect("valid Docker JSON lines");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "web");
        assert_eq!(rows[1].state, ContainerState::Stopped);
    }

    #[test]
    fn search_matches_name_id_and_image_case_insensitively() {
        let rows = vec![container(
            "abc123",
            "web-nginx",
            "nginx:1.27",
            ContainerState::Running,
        )];
        assert_eq!(
            filter_containers(&rows, "NGINX", ContainerFilter::All).len(),
            1
        );
        assert_eq!(
            filter_containers(&rows, "ABC123", ContainerFilter::All).len(),
            1
        );
        assert!(filter_containers(&rows, "postgres", ContainerFilter::All).is_empty());
    }

    #[test]
    fn filters_running_and_stopped_without_mutating_snapshot() {
        let rows = vec![
            container("run", "running", "busybox", ContainerState::Running),
            container("stop", "stopped", "busybox", ContainerState::Stopped),
        ];
        assert_eq!(
            filter_containers(&rows, "", ContainerFilter::Running).len(),
            1
        );
        assert_eq!(
            filter_containers(&rows, "", ContainerFilter::Stopped).len(),
            1
        );
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn classifies_not_installed_permission_daemon_and_parse_failures() {
        assert_eq!(
            classify_failure(&exec_failure("docker: command not found")),
            DockerErrorKind::NotInstalled
        );
        assert_eq!(
            classify_failure(&exec_failure("permission denied while trying to connect")),
            DockerErrorKind::PermissionDenied
        );
        assert_eq!(
            classify_failure(&exec_failure("Cannot connect to the Docker daemon")),
            DockerErrorKind::DaemonUnavailable
        );
        assert_eq!(
            classify_failure(&exec_failure("unexpected failure")),
            DockerErrorKind::CommandFailed
        );
    }

    #[test]
    fn parses_images_and_filters_repository_tag_and_id() {
        let rows = parse_image_rows(
            r#"{"ID":"sha256:abc","Repository":"nginx","Tag":"Latest","Size":"42MB","CreatedAt":"yesterday"}"#,
        )
        .expect("valid Docker image JSON");
        assert_eq!(filter_images(&rows, "NGINX").len(), 1);
        assert_eq!(filter_images(&rows, "latest").len(), 1);
        assert_eq!(filter_images(&rows, "SHA256:ABC").len(), 1);
    }

    #[test]
    fn parses_inspect_details_without_exposing_environment() {
        let stdout = r#"[{"Id":"container-id","Config":{"Image":"nginx:1.27","Cmd":["nginx","-g","daemon off;"],"Env":["SECRET=do-not-copy"]},"Created":"2026-09-08T10:00:00Z","NetworkSettings":{"Ports":{"80/tcp":[{"HostPort":"8080"}]},"Networks":{"default":{"IPAddress":"172.20.0.2"}}},"Mounts":[{"Source":"/data","Destination":"/var/lib"}]}]"#;
        let detail = parse_container_detail(stdout).expect("valid container inspect JSON");
        assert_eq!(detail.id, "container-id");
        assert_eq!(detail.image, "nginx:1.27");
        assert!(detail.command.contains("nginx"));
        assert!(detail.mounts.contains("/var/lib"));
        assert!(detail.networks.contains("172.20.0.2"));
        assert!(!detail.command.contains("SECRET"));
        assert!(!detail.mounts.contains("SECRET"));
        assert!(!detail.networks.contains("SECRET"));
    }

    #[test]
    fn parses_first_object_after_non_object_inspect_entry() {
        let stdout = r#"[null,{"Id":"container-id","Config":{"Image":"nginx:1.27"}}]"#;
        let detail = parse_container_detail(stdout).expect("first object in inspect JSON");
        assert_eq!(detail.id, "container-id");
    }

    #[test]
    fn rejects_malformed_list_and_empty_inspect_json_as_parse_failures() {
        assert_eq!(
            parse_container_rows("not json").unwrap_err().kind,
            DockerErrorKind::ParseFailed
        );
        assert_eq!(
            parse_image_detail("[]").unwrap_err().kind,
            DockerErrorKind::ParseFailed
        );
    }

    fn container(
        id: &str,
        name: &str,
        image: &str,
        state: ContainerState,
    ) -> DockerContainerSummary {
        DockerContainerSummary {
            id: id.into(),
            name: name.into(),
            image: image.into(),
            state,
            status: String::new(),
            created: String::new(),
            ports: String::new(),
        }
    }

    fn exec_failure(stderr: &str) -> DockerExecResult {
        DockerExecResult {
            stderr: stderr.into(),
            exit_code: Some(1),
            ..DockerExecResult::default()
        }
    }
}
