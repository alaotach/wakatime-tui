# HackaTime-TUI

A Terminal User Interface (TUI) for monitoring your coding activity stats from [Hackatime](https://github.com/hackclub/hackatime) (hackclub time tracking tool originally wakatime). Built with Rust using [Ratatui](https://ratatui.rs/).

## Features

### Dashboard (Main View)

### Projects View

### Leaderboard View

### Day View

### Setup & Onboarding

## Installation

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install)
- Hackatime server running and accessible
- API key from your Hackatime instance

### From Source

#### Windows (PowerShell)
```powershell
git clone https://github.com/alaotach/WakaTime-TUI.git
cd WakaTime-TUI
cargo build --release
.\target\release\WakaTime-TUI.exe
```

#### macOS & Linux (Bash/Zsh)
```bash
git clone https://github.com/alaotach/WakaTime-TUI.git
cd WakaTime-TUI
cargo build --release
./target/release/WakaTime-TUI
```

### Configuration

Create a configuration file at one of these locations:
- `~/.wakatime.cfg` (recommended)
- `$HACKATIME_CONFIG` environment variable path
- `%USERPROFILE%\.wakatime.cfg` (Windows)

**Configuration File Template:**
```ini
[settings]
api_url = https://hackatime.hackclub.com
api_key = your_api_key_here
heartbeat_rate_limit_seconds = 120
```

### Environment Variables (Alternative Setup)

Set these environment variables before running:
```bash
export HACKATIME_API_URL="https://hackatime.hackclub.com"
export HACKATIME_API_KEY="your_api_key_here"
```

### Activity Heatmap
- The 40×9 grid shows coding activity patterns across weeks
- Darker cells = lower activity
- Brighter green cells = higher activity
- Helps identify your coding rhythms and productivity patterns

### Hourly Chart (Day View)
- Shows time spent coding in each hour (0-23)
- Height of bars indicates activity level
- Current hour highlighted in bright pink when viewing today
- Useful for analyzing daily productivity and identifying peak coding times

## Running the Application

### Standard Run
```bash
cargo run --release
```

### Development Mode
```bash
cargo run
```

### Headless Testing (Advanced)
```bash
RUST_LOG=debug cargo run --release
```

## Development

### Project Structure
```
.
├── src/
│   ├── main.rs          # TUI event loop and rendering
│   ├── config.rs        # Configuration management
│   └── api/
│       ├── mod.rs       # API module exports
│       └── wakatime.rs  # Hackatime API client
├── Cargo.toml           # Project manifest
└── Readme.md            # This file
```
### Testing
```bash
cargo check             # Quick syntax check
cargo test              # Run all tests
```

### Code Quality
```bash
cargo fmt               # Format code
cargo clippy            # Lint check
```

## Contributing

We welcome contributions! This project is open to anyone interested in improving the WakaTime TUI experience.

### How to Contribute

1. **Fork the repository**
   ```bash
   git clone https://github.com/alaotach/WakaTime-TUI.git
   cd WakaTime-TUI
   ```

2. **Create a feature branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

3. **Make your changes**
   - Write clean, idiomatic Rust code
   - Follow the existing code style
   - Add comments for complex logic
   - Test thoroughly before submitting

4. **Commit your changes**
   ```bash
   git commit -m "Add: your feature description"
   ```

5. **Push to your fork**
   ```bash
   git push origin feature/your-feature-name
   ```

6. **Open a Pull Request**
   - Describe what your PR does
   - Reference any related issues
   - Include test results
   - Request reviews from maintainers

### Contribution Guidelines

- **Code Style**: Follow Rust conventions and use `cargo fmt`
- **Testing**: Add tests for new features
- **Documentation**: Update README for user-facing changes
- **Commits**: Use clear, descriptive commit messages
- **Issues**: Check existing issues before starting new work
- **Communication**: Discuss major changes in an issue first

### Areas for Contribution

- Bug fixes
- New features
- Performance optimizations
- UI/UX enhancements
- Additional tests
- API integration improvements

### Development Setup

1. Install Rust (latest stable): https://rustup.rs/
2. Clone the repository
3. Run `cargo build` to download dependencies
4. Make changes and test with `cargo run`
5. Validate with `cargo fmt`, `cargo clippy`, and `cargo test`

### Reporting Issues

Found a bug or have a suggestion? Please [open an issue](https://github.com/alaotach/WakaTime-TUI/issues/new) with:
- Clear description of the problem
- Steps to reproduce (if applicable)
- Expected vs actual behavior
- Your environment (OS, Rust version, etc.)
- Screenshots if relevant

## Privacy & Security

- Credentials are stored locally in `~/.wakatime.cfg` with appropriate file permissions
- API keys are only sent to your configured Hackatime instance
- No data is collected or transmitted to third parties
- Source code is open for security audit

## License

This project is licensed under the [MIT License](LICENSE).

**Happy coding!** Track your productivity with WakaTime-TUI.