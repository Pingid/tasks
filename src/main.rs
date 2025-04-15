use clap::Parser;
use colored::{ColoredString, Colorize};
use futures::future::join_all;
use std::{
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
    sync::Notify,
};

/// Run multiple commands in parallel
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
struct Args {
    /// Commands to run in parallel. Each command can optionally have a prefix in the format [prefix]:command
    #[arg(
        required = true,
        help = "Commands to run. Use format '[prefix]:command' for prefixed output"
    )]
    commands: Vec<String>,

    /// Dont include prefix in output
    #[arg(long)]
    no_prefix: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let control = TaskControl::new();

    let inner = control.clone();
    ctrlc::set_handler(move || inner.stop()).unwrap();

    let tasks = (0..args.commands.len())
        .map(|i| Task::new(control.clone(), Arc::new(args.clone()), i))
        .map(|task| tokio::spawn(async move { task.start().await }))
        .collect::<Vec<_>>();

    let results = join_all(tasks).await;
    for result in results {
        if let Err(e) = result {
            eprintln!("Task error: {}", e);
        }
    }
}

#[derive(Debug, Clone)]
struct TaskControl {
    notify: Arc<Notify>,
    stopped: Arc<AtomicBool>,
}

impl TaskControl {
    fn new() -> Self {
        Self {
            notify: Arc::new(Notify::new()),
            stopped: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
        self.notify.notify_waiters();
    }

    pub async fn is_stopped(&self) -> bool {
        let _ = self.notify.notified().await;
        let stopped = self.stopped.load(Ordering::Relaxed);
        stopped
    }
}

#[derive(Debug, Clone)]
struct Task {
    control: TaskControl,
    args: Arc<Args>,
    index: usize,
}

impl Task {
    fn new(control: TaskControl, args: Arc<Args>, index: usize) -> Self {
        Self {
            control,
            index,
            args,
        }
    }

    async fn start(&self) -> Result<(), std::io::Error> {
        let (command, prefix) = parse_command(&self.args.commands[self.index]);

        let mut cmd = bash_command(command);
        let mut child = cmd.spawn()?;

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let prefix = match &prefix {
            Some(prefix) => Some(colorize(prefix, self.index).to_string()),
            None => None,
        };

        let stdout_handle = command_print(stdout, prefix.clone());
        let stderr_handle = command_print(stderr, prefix);
        let stopped = self.control.is_stopped();

        let should_exit = tokio::select! {
            _ = child.wait() => false,
            _ = stopped => true,
            _ = stdout_handle => false,
            _ = stderr_handle => false,
        };

        if should_exit {
            child.kill().await?;
        }

        Ok(())
    }
}

fn parse_command<'a>(value: &'a str) -> (&'a str, Option<&'a str>) {
    if value.starts_with('[') {
        if let Some(end_bracket) = value.find(']') {
            if value[end_bracket..].starts_with("]:") {
                let prefix = &value[1..end_bracket]; // Only what's inside the brackets
                let command = &value[end_bracket + 2..]; // Skips over "]:"
                return (command, Some(prefix));
            }
        }
    }
    (value, None)
}

fn bash_command(c: &str) -> Command {
    let shell = "bash";
    let mut cmd = Command::new(shell);
    cmd.args(&["-c", c]);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::null());
    cmd.env("FORCE_COLOR", "1");
    cmd
}

fn colorize(str: &str, i: usize) -> ColoredString {
    let i = i % 6;
    match i {
        0 => Colorize::red(str),
        1 => Colorize::green(str),
        2 => Colorize::yellow(str),
        3 => Colorize::blue(str),
        4 => Colorize::magenta(str),
        _ => Colorize::cyan(str),
    }
}

async fn command_print<C: AsyncRead>(
    reader: C,
    prefix: Option<String>,
) -> Result<(), std::io::Error> {
    let mut line = String::new();
    let mut reader = Box::pin(BufReader::new(reader));
    while let Ok(n) = reader.read_line(&mut line).await {
        if n > 0 {
            match &prefix {
                Some(prefix) => print!("[{}] {}\r\n", prefix, &line.trim()),
                None => print!("{}\r\n", &line.trim()),
            }
            line.clear();
        }
    }
    Ok(())
}
