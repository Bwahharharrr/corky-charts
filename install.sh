#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=======================================================${NC}"
echo -e "${GREEN}Corky Charts Installer${NC}"
echo -e "${BLUE}=======================================================${NC}"

# Build first as the user
if [ ! -f "$(pwd)/target/release/corky-charts" ]; then
    echo -e "\n${BLUE}Step 1: Building the executable...${NC}"
    cargo build --release
    if [ $? -ne 0 ]; then
        echo -e "${RED}Error: Failed to build the executable.${NC}"
        exit 1
    fi
    echo -e "${GREEN}Build successful!${NC}"
fi

# Check if running as root for the installation steps
IS_ROOT=0
if [ $(id -u) -eq 0 ]; then
    IS_ROOT=1
else
    echo -e "\n${YELLOW}Installation requires root privileges for system service setup.${NC}"
    echo -e "${YELLOW}Continuing with sudo for installation steps...${NC}"
    exec sudo "$0" "$@"
    exit 1  # This line should not be reached
fi

# Capture the actual user who ran the script, even if via sudo
REAL_USER=${SUDO_USER:-$USER}
REAL_HOME=$(eval echo ~$REAL_USER)

# Variables
CORKY_DIR="$REAL_HOME/.corky"
CORKY_BIN_DIR="$CORKY_DIR/bin"
CORKY_CONFIG_FILE="$CORKY_DIR/config.toml"
EXECUTABLE_NAME="corky-charts"
EXECUTABLE_PATH="$CORKY_BIN_DIR/$EXECUTABLE_NAME"
DEFAULT_OUTPUT_DIR="/tmp"

# No log storage needed for corky-charts

# Function to check if a directory exists, create it if not
create_dir_if_not_exists() {
    if [ ! -d "$1" ]; then
        echo -e "${YELLOW}Creating directory: $1${NC}"
        mkdir -p "$1"
        
        # Set appropriate ownership
        if [ $IS_ROOT -eq 1 ]; then
            if [[ "$1" == "$REAL_HOME"* ]]; then
                # This is in the user's home directory, should be owned by the user
                chown -R $REAL_USER:$(id -gn $REAL_USER) "$1"
                chmod 755 "$1"
            else
                # This is a system directory, should be owned by root
                chown root:root "$1"
                chmod 755 "$1"
            fi
        fi
    else
        echo -e "${GREEN}Directory already exists: $1${NC}"
    fi
}

# Step 1: Create necessary directories
echo -e "\n${BLUE}Step 1: Creating necessary directories...${NC}"
create_dir_if_not_exists "$CORKY_DIR"
create_dir_if_not_exists "$CORKY_BIN_DIR"

# Step 2: Copy the executable to the bin directory
echo -e "\n${BLUE}Step 2: Installing the executable...${NC}"
cp "$(pwd)/target/release/corky-charts" "$EXECUTABLE_PATH"
chmod 755 "$EXECUTABLE_PATH"
chown $REAL_USER:$(id -gn $REAL_USER) "$EXECUTABLE_PATH" || true
echo -e "${GREEN}Executable installed at: $EXECUTABLE_PATH${NC}"

# Step 3: Configure output directory in config.toml
echo -e "\n${BLUE}Step 3: Setting up configuration...${NC}"

# Check if config file exists
CHARTS_CONFIG_UPDATED=0
if [ -f "$CORKY_CONFIG_FILE" ]; then
    echo -e "${YELLOW}Checking existing config file...${NC}"
    
    # Check if [charts] section exists
    if grep -q "\[charts\]" "$CORKY_CONFIG_FILE"; then
        echo -e "${GREEN}Found existing [charts] section in config.${NC}"
        
        # Check if directory is set
        if grep -q "^directory\s*=" "$CORKY_CONFIG_FILE"; then
            echo -e "${GREEN}Output directory already configured in config file.${NC}"
        else
            # Add directory setting to existing [charts] section
            sed -i "/\[charts\]/a directory = \"$DEFAULT_OUTPUT_DIR\"" "$CORKY_CONFIG_FILE"
            echo -e "${GREEN}Added output directory setting to existing [charts] section.${NC}"
            CHARTS_CONFIG_UPDATED=1
        fi
    else
        # Add [charts] section
        echo -e "\n[charts]\ndirectory = \"$DEFAULT_OUTPUT_DIR\"" >> "$CORKY_CONFIG_FILE"
        echo -e "${GREEN}Added [charts] section to config file.${NC}"
        CHARTS_CONFIG_UPDATED=1
    fi
else
    # Create new config file
    echo -e "${YELLOW}Creating new config file...${NC}"
    cat > "$CORKY_CONFIG_FILE" << EOF
# Corky Configuration File

[charts]
directory = "$DEFAULT_OUTPUT_DIR"
EOF
    chown $REAL_USER:$(id -gn $REAL_USER) "$CORKY_CONFIG_FILE" || true
    echo -e "${GREEN}Created new config file with [charts] section.${NC}"
    CHARTS_CONFIG_UPDATED=1
fi

if [ $CHARTS_CONFIG_UPDATED -eq 1 ]; then
    echo -e "${GREEN}Output directory for chart images: $DEFAULT_OUTPUT_DIR${NC}"
fi

# Step 4: Setup PATH if needed (only if not installing to /usr/local/bin)
echo -e "\n${BLUE}Step 4: Checking PATH configuration...${NC}"
if [[ "$CORKY_BIN_DIR" != "/usr/local/bin" ]]; then
    if [[ ":$PATH:" != *":$CORKY_BIN_DIR:"* ]]; then
        echo -e "${YELLOW}Adding $CORKY_BIN_DIR to PATH...${NC}"
        
        # Determine which shell configuration file to use
        SHELL_CONFIG=""
        USER_SHELL_CONFIG=""
        
        if [ -f "$REAL_HOME/.bashrc" ]; then
            USER_SHELL_CONFIG="$REAL_HOME/.bashrc"
        elif [ -f "$REAL_HOME/.zshrc" ]; then
            USER_SHELL_CONFIG="$REAL_HOME/.zshrc"
        elif [ -f "$REAL_HOME/.profile" ]; then
            USER_SHELL_CONFIG="$REAL_HOME/.profile"
        fi
        
        if [ -n "$USER_SHELL_CONFIG" ]; then
            # Add the PATH to the actual user's shell config
            if [ $IS_ROOT -eq 1 ]; then
                # Need to append to the file as the user
                su - $REAL_USER -c "echo '# Added by Corky Charts Installer' >> '$USER_SHELL_CONFIG'"
                su - $REAL_USER -c "echo 'export PATH=\"\$PATH:$CORKY_BIN_DIR\"' >> '$USER_SHELL_CONFIG'"
            else
                echo "# Added by Corky Charts Installer" >> "$USER_SHELL_CONFIG"
                echo "export PATH=\"\$PATH:$CORKY_BIN_DIR\"" >> "$USER_SHELL_CONFIG"
            fi
            echo -e "${GREEN}Added $CORKY_BIN_DIR to PATH in $USER_SHELL_CONFIG${NC}"
            echo -e "${YELLOW}Please run 'source $USER_SHELL_CONFIG' or restart your terminal to apply changes${NC}"
        else
            echo -e "${RED}Could not find a shell configuration file to update.${NC}"
            echo -e "${YELLOW}Please manually add the following line to your shell configuration:${NC}"
            echo -e "${YELLOW}export PATH=\"\$PATH:$CORKY_BIN_DIR\"${NC}"
        fi
    else
        echo -e "${GREEN}$CORKY_BIN_DIR is already in PATH${NC}"
    fi
else
    echo -e "${GREEN}/usr/local/bin is already in standard PATH, no changes needed${NC}"
fi

# Step 5: Setting up service...
echo -e "\n${BLUE}Step 5: Setting up service...${NC}"

# Default to system service for root and user service for non-root
if [ $IS_ROOT -eq 1 ]; then
    # Create a system-wide service but using user config
    SYSTEMD_SERVICE_FILE="/etc/systemd/system/corky-charts.service"
    echo -e "${YELLOW}Creating system-wide systemd service at: $SYSTEMD_SERVICE_FILE${NC}"

    cat > "$SYSTEMD_SERVICE_FILE" << EOF
[Unit]
Description=Corky Charts Service
After=network.target

[Service]
ExecStart=$EXECUTABLE_PATH
WorkingDirectory=$(pwd)
# Run as root but use the user's configuration
User=$REAL_USER
Restart=on-failure
RestartSec=5s
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF
    chmod 644 "$SYSTEMD_SERVICE_FILE"
    
    # Automatically enable and start the service
    echo -e "${YELLOW}Enabling and starting the service...${NC}"
    systemctl daemon-reload
    systemctl enable corky-charts.service
    systemctl start corky-charts.service
    
    echo -e "${GREEN}Service installed and started.${NC}"
    echo -e "${YELLOW}To check the status: systemctl status corky-charts.service${NC}"
    echo -e "${YELLOW}To stop the service: systemctl stop corky-charts.service${NC}"
else
    # Create a user service
    SYSTEMD_USER_DIR="$REAL_HOME/.config/systemd/user"
    create_dir_if_not_exists "$SYSTEMD_USER_DIR"
    
    SYSTEMD_SERVICE_FILE="$SYSTEMD_USER_DIR/corky-charts.service"
    echo -e "${YELLOW}Creating systemd user service at: $SYSTEMD_SERVICE_FILE${NC}"

    cat > "$SYSTEMD_SERVICE_FILE" << EOF
[Unit]
Description=Corky Charts Service
After=network.target

[Service]
ExecStart=$EXECUTABLE_PATH
WorkingDirectory=$(pwd)
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=default.target
EOF
    
    # Set proper ownership of the service file
    chown $REAL_USER:$(id -gn $REAL_USER) "$SYSTEMD_SERVICE_FILE"
    
    echo -e "${YELLOW}You can now start and enable the service with:${NC}"
    echo -e "${YELLOW}systemctl --user daemon-reload${NC}"
    echo -e "${YELLOW}systemctl --user enable --now corky-charts.service${NC}"
    echo -e "${YELLOW}To check the status: systemctl --user status corky-charts.service${NC}"
fi

echo -e "\n${GREEN}Installation completed successfully!${NC}"
echo -e "${GREEN}Executable: $EXECUTABLE_PATH${NC}"
echo -e "${GREEN}Configuration: $CORKY_CONFIG_FILE${NC}"
echo -e "${BLUE}=======================================================${NC}"
