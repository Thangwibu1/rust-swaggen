//! Allowlisted diagnostics for the P00 server-side probe. Never format driver errors.
use mongodb::error::{Error, ErrorKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Diagnosis {
    pub status: &'static str,
    pub category: &'static str,
}

pub fn classify(error: Option<&Error>, stage: &str) -> Diagnosis {
    let (status, category) = if let Some(error) = error {
        if error.contains_label("UnknownTransactionCommitResult") {
            ("BLOCKED", "UNKNOWN_COMMIT_RESULT")
        } else {
            match error.kind.as_ref() {
                ErrorKind::Io(source) if source.raw_os_error() == Some(10013) => {
                    ("BLOCKED", "SOCKET_PERMISSION")
                }
                ErrorKind::Io(_) => ("BLOCKED", "NETWORK_IO"),
                ErrorKind::DnsResolve { .. } => ("BLOCKED", "DNS_RESOLUTION"),
                // Selection failure has several possible causes. Do not guess DNS or auth.
                ErrorKind::ServerSelection { .. } => {
                    ("BLOCKED", "CONNECTIVITY_OR_SERVER_SELECTION")
                }
                ErrorKind::InvalidArgument { .. } | ErrorKind::InvalidTlsConfig { .. } => {
                    ("BLOCKED", "CONFIGURATION")
                }
                ErrorKind::Authentication { .. } => ("FAIL", "AUTHENTICATION"),
                ErrorKind::Command(command) if command.code == 13 => ("FAIL", "AUTHORIZATION"),
                ErrorKind::SessionsNotSupported => ("BLOCKED", "SESSIONS_UNSUPPORTED"),
                _ => ("FAIL", "DRIVER_OPERATION"),
            }
        }
    } else if stage == "configuration" {
        ("BLOCKED", "CONFIGURATION")
    } else if stage == "topology" {
        ("BLOCKED", "TOPOLOGY_UNVERIFIED")
    } else {
        ("FAIL", "PROBE_OPERATION")
    };
    Diagnosis { status, category }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_permission_is_not_reported_as_database_transaction_failure() {
        let error = Error::from(std::io::Error::from_raw_os_error(10013));
        assert_eq!(
            classify(Some(&error), "topology"),
            Diagnosis {
                status: "BLOCKED",
                category: "SOCKET_PERMISSION"
            }
        );
    }

    #[test]
    fn diagnostic_output_does_not_include_private_error_message() {
        let marker = "private credential and endpoint marker";
        let error = Error::from(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            marker,
        ));
        let result = classify(Some(&error), "topology");
        assert_eq!(result.category, "NETWORK_IO");
        assert!(!format!("{result:?}").contains(marker));
    }

    #[tokio::test]
    async fn malformed_uri_is_configuration_without_network_probe() {
        let error = mongodb::options::ClientOptions::parse("not-a-mongo-uri")
            .await
            .unwrap_err();
        assert_eq!(
            classify(Some(&error), "configuration").category,
            "CONFIGURATION"
        );
        assert_eq!(classify(None, "configuration").status, "BLOCKED");
    }
}
