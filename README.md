# Tasks CLI

A command-line tool for running multiple commands concurrently.

## Installation

```bash
cargo install https://github.com/Pingid/tasks
```

## Usage

The tool accepts one or more commands as arguments. Each command can optionally have a prefix in square brackets.

### Basic Usage

```bash
tasks "command1" "command2" "command3"
```

### With Custom Prefixes

You can add custom prefixes to commands using square brackets:

```bash
tasks "[server] npm start" "[client] npm run dev" "[db] docker-compose up"
```

## License

This project is open source and available under the MIT License.
