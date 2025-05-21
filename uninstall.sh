#!/bin/bash
# Removed the 'set -e' to prevent premature exits

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=======================================================${NC}"
echo -e "${RED}Corky Charts Uninstaller${NC}"
echo -e "${BLUE}=======================================================${NC}"

# Check if running as root
IS_ROOT=0
if [ $(id -u) -eq 0 ]; then
    IS_ROOT=1
fi

# Capture the actual user who ran the script, even if via sudo
REAL_USER=${SUDO_USER:-$USER}
REAL_HOME=$(eval echo ~$REAL_USER)

# Variables for installed locations
CORKY_DIR="$REAL_HOME/.corky"
CORKY_BIN_DIR="$CORKY_DIR/bin"
CORKY_CONFIG_FILE="$CORKY_DIR/config.toml"
EXECUTABLE_NAME="corky-charts"
EXECUTABLE_PATH="$CORKY_BIN_DIR/$EXECUTABLE_NAME"
USER_SERVICE_FILE="$REAL_HOME/.config/systemd/user/corky-charts.service"
SYSTEM_SERVICE_FILE="/etc/systemd/system/corky-charts.service"

# Default settings
KEEP_CONFIG_FLAG=1

# Function to display files that will be removed
show_removal_info() {
    echo -e "${BLUE}The following items will be removed:${NC}"
    
    # Always show user files
    echo -e "${YELLOW}User-level files:${NC}"
    [ -f "$EXECUTABLE_PATH" ] && echo "  - $EXECUTABLE_PATH (the executable only)"
    [ -f "$USER_SERVICE_FILE" ] && echo "  - $USER_SERVICE_FILE"
    
    # Only show system files if root
    if [ $IS_ROOT -eq 1 ]; then
        echo -e "${YELLOW}System-level files:${NC}"
        [ -f "$SYSTEM_SERVICE_FILE" ] && echo "  - $SYSTEM_SERVICE_FILE"
    else
        echo -e "${YELLOW}System-level files (requires sudo to remove):${NC}"
        [ -f "$SYSTEM_SERVICE_FILE" ] && echo "  - $SYSTEM_SERVICE_FILE"
    fi
    
    # Check for PATH modifications
    if grep -q "CORKY_BIN_DIR" "$HOME/.bashrc" 2>/dev/null; then
        echo "  - PATH modification in $HOME/.bashrc"
    fi
    if grep -q "CORKY_BIN_DIR" "$HOME/.zshrc" 2>/dev/null; then
        echo "  - PATH modification in $HOME/.zshrc"
    fi
    if grep -q "CORKY_BIN_DIR" "$HOME/.profile" 2>/dev/null; then
        echo "  - PATH modification in $HOME/.profile"
    fi
    
    # Show what will NOT be removed
    echo -e "\n${GREEN}The following shared resources will be preserved:${NC}"
    echo "  - $CORKY_DIR (main Corky directory)"
    echo "  - $CORKY_CONFIG_FILE (with [charts] section removed)"
}

# Show what will be removed
show_removal_info

# Ask for confirmation
echo ""
echo -e "${RED}WARNING: This will remove the Corky Charts executable and service.${NC}"
echo -e "${YELLOW}Note: The [charts] section in config.toml will be removed, but all other sections will be preserved.${NC}"
read -p "Do you want to proceed with uninstallation? (y/n): " CONFIRM
if [[ ! "$CONFIRM" =~ ^[Yy]$ ]]; then
    echo -e "${GREEN}Uninstallation aborted.${NC}"
    exit 0
fi

# Ask about complete config removal
read -p "Do you want to completely remove the config file? (not recommended if you use other Corky apps) (y/n, default: n): " REMOVE_CONFIG
if [[ "$REMOVE_CONFIG" =~ ^[Yy]$ ]]; then
    KEEP_CONFIG_FLAG=0
    echo -e "${RED}Config file will be completely removed.${NC}"
else
    echo -e "${YELLOW}Config file will be preserved with the [charts] section removed.${NC}"
fi

# Step 1: Stop and disable services
echo -e "\n${BLUE}Step 1: Stopping services...${NC}"

# Try to stop and disable both user and system services
if [ $IS_ROOT -eq 1 ]; then
    # For root, try to stop and disable system service
    if systemctl is-active --quiet corky-charts.service; then
        echo -e "${YELLOW}Stopping system service...${NC}"
        systemctl stop corky-charts.service
    fi
    if systemctl is-enabled --quiet corky-charts.service 2>/dev/null; then
        echo -e "${YELLOW}Disabling system service...${NC}"
        systemctl disable corky-charts.service
    fi
else
    # For regular user, try both user and system services
    if systemctl --user is-active --quiet corky-charts.service 2>/dev/null; then
        echo -e "${YELLOW}Stopping user service...${NC}"
        systemctl --user stop corky-charts.service
    fi
    if systemctl --user is-enabled --quiet corky-charts.service 2>/dev/null; then
        echo -e "${YELLOW}Disabling user service...${NC}"
        systemctl --user disable corky-charts.service
    fi
    
    # Inform about system service which needs root
    if systemctl is-active --quiet corky-charts.service 2>/dev/null || 
       systemctl is-enabled --quiet corky-charts.service 2>/dev/null; then
        echo -e "${YELLOW}A system-level service is running but requires root to stop.${NC}"
        echo -e "${YELLOW}Please run with sudo to complete the full uninstallation.${NC}"
    fi
fi

# 2. Remove binaries and configuration 
echo -e "\n${BLUE}Step 2: Removing executable...${NC}"

# Remove executable
if [ -f "$EXECUTABLE_PATH" ]; then
    echo -e "${YELLOW}Removing executable...${NC}"
    rm -f "$EXECUTABLE_PATH"
    echo -e "${GREEN}Removed: $EXECUTABLE_PATH${NC}"
else
    echo -e "${YELLOW}Executable not found: $EXECUTABLE_PATH${NC}"
fi

# Note about config preservation
echo -e "${GREEN}Preserving shared config file: $CORKY_CONFIG_DIR${NC}"
echo -e "${YELLOW}This file may be used by other Corky services.${NC}"

# 3. Remove or clean config file
CONFIG_REMOVED=0
CHARTS_SECTION_REMOVED=0

echo -e "\n${BLUE}Step 3: Updating configuration...${NC}"

if [ $KEEP_CONFIG_FLAG -eq 0 ]; then
    # Complete config removal requested
    if [ -f "$CORKY_CONFIG_FILE" ]; then
        echo -e "${YELLOW}Removing config file...${NC}"
        rm -f "$CORKY_CONFIG_FILE"
        CONFIG_REMOVED=1
    fi
else
    # Just remove the [charts] section
    if [ -f "$CORKY_CONFIG_FILE" ]; then
        if grep -q "\[charts\]" "$CORKY_CONFIG_FILE"; then
            echo -e "${YELLOW}Removing [charts] section from config file...${NC}"
            
            # Create temp file
            TEMP_FILE=$(mktemp)
            
            # Process the file to remove the [charts] section and its contents
            awk 'BEGIN {skip=0;} 
                 /^\[charts\]/ {skip=1; next;} 
                 /^\[.*\]/ {skip=0;} 
                 !skip {print;}' "$CORKY_CONFIG_FILE" > "$TEMP_FILE"
            
            # Remove any trailing blank lines
            sed -i -e :a -e '/^\n*$/{$d;N;ba' -e '}' "$TEMP_FILE"
            
            # Copy back to original
            mv "$TEMP_FILE" "$CORKY_CONFIG_FILE"
            chown $REAL_USER:$(id -gn $REAL_USER) "$CORKY_CONFIG_FILE" || true
            
            CHARTS_SECTION_REMOVED=1
        else
            echo -e "${GREEN}No [charts] section found in config file.${NC}"
        fi
    else
        echo -e "${GREEN}No config file found, nothing to update.${NC}"
    fi
fi

# 4. Remove service files
echo -e "\n${BLUE}Step 4: Removing service files...${NC}"

# Remove systemd service files based on permissions
if [ $IS_ROOT -eq 1 ]; then
    if [ -f "$SYSTEM_SERVICE_FILE" ]; then
        echo -e "${YELLOW}Removing system service file...${NC}"
        rm -f "$SYSTEM_SERVICE_FILE"
        systemctl daemon-reload
    fi
fi

# Remove user service file
if [ -f "$USER_SERVICE_FILE" ]; then
    echo -e "${YELLOW}Removing user service file...${NC}"
    rm -f "$USER_SERVICE_FILE"
    
    # Only need to reload user daemon if running as non-root
    if [ $IS_ROOT -eq 0 ]; then
        systemctl --user daemon-reload 2>/dev/null || true
    fi
fi

# 5. Remove PATH modifications (comment out added lines rather than removing)
echo -e "\n${BLUE}Step 5: Cleaning up PATH modifications...${NC}"
for RC_FILE in "$HOME/.bashrc" "$HOME/.zshrc" "$HOME/.profile"; do
    if [ -f "$RC_FILE" ] && grep -q "Added by Corky Charts Installer" "$RC_FILE"; then
        echo -e "${YELLOW}Commenting out PATH modifications in $RC_FILE...${NC}"
        sed -i '/Added by Corky Charts Installer/s/^/# REMOVED: /' "$RC_FILE"
        sed -i '/export PATH=.*CORKY_BIN_DIR/s/^/# REMOVED: /' "$RC_FILE"
    fi
done

# 6. Shared resources information
echo -e "\n${BLUE}Step 6: Shared resources information...${NC}"
echo -e "${GREEN}The following shared resources were preserved:${NC}"
echo -e "${YELLOW}  - $CORKY_BIN_DIR (bin directory)${NC}"
echo -e "${YELLOW}  - $CORKY_DIR (main Corky directory)${NC}"

if [ $KEEP_CONFIG_FLAG -eq 1 ] && [ $CONFIG_REMOVED -eq 0 ]; then
    echo -e "${YELLOW}  - $CORKY_CONFIG_FILE (config file)${NC}"
fi

# 7. Final summary
echo -e "\n${GREEN}Uninstallation completed!${NC}"
echo -e "${GREEN}Corky Charts has been removed from your system.${NC}"
echo -e "${GREEN}Shared resources were preserved for other Corky services.${NC}"

echo -e "${BLUE}=======================================================${NC}"
echo -e "${YELLOW}You may need to restart your terminal or run 'source ~/.bashrc'${NC}"
echo -e "${YELLOW}to complete the uninstallation process.${NC}"
echo -e "${BLUE}=======================================================${NC}"
