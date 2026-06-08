use std::process::Command;

pub struct CmdResult {
    pub output: String,
    pub code: i32,
}

impl CmdResult {
    pub fn success(&self) -> bool {
        self.code == 0
    }
}

pub fn run_cmd(args: &[&str]) -> CmdResult {
    if args.is_empty() {
        return CmdResult { output: String::new(), code: 1 };
    }
    match Command::new(args[0]).args(&args[1..]).output() {
        Ok(out) => {
            let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
            combined.push_str(&String::from_utf8_lossy(&out.stderr));
            CmdResult {
                output: combined.trim().to_string(),
                code: out.status.code().unwrap_or(1),
            }
        }
        Err(e) => CmdResult { output: e.to_string(), code: 1 },
    }
}

pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

pub fn require_sudo() {
    if !is_root() {
        let args: Vec<String> = std::env::args().collect();
        eprintln!("This command needs root. Re-run with: sudo {}", args.join(" "));
        std::process::exit(1);
    }
}

pub fn run_as_user(args: &[&str]) -> CmdResult {
    if is_root() {
        if let Ok(sudo_user) = std::env::var("SUDO_USER") {
            let mut full: Vec<String> = vec!["sudo".into(), "-u".into(), sudo_user];
            full.extend(args.iter().map(|s| s.to_string()));
            let refs: Vec<&str> = full.iter().map(|s| s.as_str()).collect();
            return run_cmd(&refs);
        }
    }
    run_cmd(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_cmd_captures_stdout() {
        let r = run_cmd(&["echo", "hello"]);
        assert!(r.success());
        assert_eq!(r.output, "hello");
    }

    #[test]
    fn run_cmd_nonexistent_binary() {
        let r = run_cmd(&["__nonexistent_binary_xyz__"]);
        assert!(!r.success());
    }

    #[test]
    fn run_cmd_empty_args() {
        let r = run_cmd(&[]);
        assert!(!r.success());
    }
}
