# xfer - Simple File Transfer Tool
A lightweight, CLI-based file transfer utility built for terminal lovers who want to simplify file transfers without using GUI tools.

`xfer` is a Rust wrapper around powerful tools like `rsync`, `scp`, and `ssh` that simplifies the syntax and stores your credentials, making terminal-based file transfers painless.

## Features
- **Simple Syntax**: Easy-to-remember commands
- **Server Aliases**: Use short names instead of typing full addresses
- **Credentials Management**: Store SSH keys and connection details securely
- **Smart Transfers**: Automatically selects the best tool for your transfer
- **Interactive Mode**: Add and manage server configurations with ease

## Installation
```bash
# Clone the repository
git clone https://github.com/mutasim77/xfer.git
cd xfer

# Build the project
cargo build
```

## Global Installation
After building your `xfer` tool, you'll want to make it available system-wide so you can run it from any directory. Here's how to properly install it:

### Building the Release Version
First, compile an optimized release version:
```bash
# Build the release version (optimized binary)
cargo build --release
```
This creates an optimized binary at `target/release/xfer`.

### Installing System-wide
#### Option 1: User-specific installation (recommended)
```bash
# Create ~/.local/bin directory if it doesn't exist
mkdir -p ~/.local/bin

# Copy the binary
cp target/release/xfer ~/.local/bin/

# Make it executable
chmod +x ~/.local/bin/xfer
```
Then add `~/.local/bin` to your `PATH` if it's not already there:

#### For Bash users (add to `~/.bashrc`):
```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

#### For Zsh users (add to `~/.zshrc`):
```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

### Option 2: System-wide installation
This requires _admin_ privileges but installs the tool for all users:
```bash
# Copy to a location that's already in your PATH
sudo cp target/release/xfer /usr/local/bin/

# Make it executable
sudo chmod +x /usr/local/bin/xfer
```

#### Verifying the Installation
Confirm that the installation worked:
```bash
# Check if the command is recognized
which xfer

# Should output something like:
# /home/yourusername/.local/bin/xfer
# or
# /usr/local/bin/xfer

# Verify it runs
xfer --version
```
Now you can run `xfer` commands from any directory:
```bash
# List configured servers
xfer server list

# Transfer files
xfer send myfile.txt myserver:/path/to/destination/
```

### Uninstalling
If you need to remove the tool later:
```bash
# If installed in ~/.local/bin
rm ~/.local/bin/xfer

# If installed in /usr/local/bin
sudo rm /usr/local/bin/xfer
```

### Basic Commands
```bash
# Send a local file to a remote server
xfer send file.txt prod:/home/user/documents/

# Download a file from a remote server
xfer get prod:/var/log/nginx/access.log ./logs/

# Sync a directory to a remote server
xfer sync ./project/ staging:/var/www/html/

# List files on a remote server
xfer list prod:/var/log/
```

### Advanced Features
1. **Smart tool selection**: The tool automatically uses:
   - `rsync` for directory transfers (better for large directories)
   - `scp` for single file transfers
   - `ssh` for listing directories

2. **Credential management**: You never need to specify your SSH key again; it's stored in the config.

3. **Server management**:
   ```bash
   xfer server add       # Add a new server
   xfer server list      # List configured servers
   ```

The tool is designed to be easily extensible too. You can add new commands or features as your needs grow.

## License
This project is licensed under the MIT License - see the [LICENSE](./LICENSE) file for details.

Built with ❤️ by [Mut](https://www.mutasim.top/)