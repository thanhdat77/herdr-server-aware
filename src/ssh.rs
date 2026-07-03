use std::process::Command;

pub fn ssh_connect_command(target: &str) -> String {
    let target = shell_quote(target);
    format!(
        "if command -v autossh >/dev/null 2>&1; then exec autossh -M 0 -o ServerAliveInterval=10 -o ServerAliveCountMax=3 -o TCPKeepAlive=yes {target}; else exec ssh -o ServerAliveInterval=10 -o ServerAliveCountMax=3 -o TCPKeepAlive=yes {target}; fi"
    )
}

pub fn ssh_terminal_attach_command(target: &str, terminal_id: &str, takeover: bool) -> String {
    let mut args = vec![
        "terminal".to_string(),
        "attach".to_string(),
        terminal_id.to_string(),
    ];
    if takeover {
        args.push("--takeover".to_string());
    }
    let remote = remote_herdr_command(&args);
    format!(
        "exec ssh -tt {} {}",
        shell_quote(target),
        shell_quote(&remote)
    )
}

pub fn remote_herdr_command(args: &[String]) -> String {
    std::iter::once("herdr".to_string())
        .chain(args.iter().map(|arg| shell_quote(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn ssh_output(target: &str, remote_command: &str) -> Result<Vec<u8>, String> {
    let out = Command::new("ssh")
        .arg(target)
        .arg(remote_command)
        .output()
        .map_err(|err| format!("failed to run ssh: {err}"))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_command_prefers_autossh() {
        let cmd = ssh_connect_command("prod-api");
        assert!(cmd.contains("autossh -M 0"));
        assert!(cmd.contains("else exec ssh"));
        assert!(cmd.contains("'prod-api'"));
    }

    #[test]
    fn shell_quote_handles_quotes() {
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    }

    #[test]
    fn terminal_attach_uses_one_remote_terminal() {
        let cmd = ssh_terminal_attach_command("prod", "term_abc", true);
        assert_eq!(
            cmd,
            r#"exec ssh -tt 'prod' 'herdr '\''terminal'\'' '\''attach'\'' '\''term_abc'\'' '\''--takeover'\'''"#
        );
    }
}
