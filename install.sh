#!/usr/bin/env bash

# Cerberus Skill - Installer
# Installs the cerberus-skill AI agent files into ~/.claude/skills/cerberus/
# and optionally updates ~/.claude/CLAUDE.md

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
WHITE='\033[1;37m'
MAGENTA='\033[0;35m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_DIR="$SCRIPT_DIR/skill"
COMMANDS_DIR="$SCRIPT_DIR/commands"

SKILLS_DIR="${HOME}/.claude/skills"
INSTALL_PATH="${SKILLS_DIR}/cerberus"
CLAUDE_MD_PATH="${HOME}/.claude/CLAUDE.md"

print_banner() {
    echo ""
    echo -e "${MAGENTA}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${MAGENTA}║${NC}                                                               ${MAGENTA}║${NC}"
    echo -e "${MAGENTA}║${NC}   ${CYAN} ██████╗███████╗██████╗ ██████╗ ███████╗██████╗ ██╗   ██╗${NC}   ${MAGENTA}║${NC}"
    echo -e "${MAGENTA}║${NC}   ${CYAN}██╔════╝██╔════╝██╔══██╗██╔══██╗██╔════╝██╔══██╗██║   ██║${NC}   ${MAGENTA}║${NC}"
    echo -e "${MAGENTA}║${NC}   ${CYAN}██║     █████╗  ██████╔╝██████╔╝█████╗  ██████╔╝██║   ██║${NC}   ${MAGENTA}║${NC}"
    echo -e "${MAGENTA}║${NC}   ${CYAN}██║     ██╔══╝  ██╔══██╗██╔══██╗██╔══╝  ██╔══██╗██║   ██║${NC}   ${MAGENTA}║${NC}"
    echo -e "${MAGENTA}║${NC}   ${CYAN}╚██████╗███████╗██║  ██║██████╔╝███████╗██║  ██║╚██████╔╝${NC}   ${MAGENTA}║${NC}"
    echo -e "${MAGENTA}║${NC}   ${CYAN} ╚═════╝╚══════╝╚═╝  ╚═╝╚═════╝ ╚══════╝╚═╝  ╚═╝ ╚═════╝${NC}   ${MAGENTA}║${NC}"
    echo -e "${MAGENTA}║${NC}                                                               ${MAGENTA}║${NC}"
    echo -e "${MAGENTA}║${NC}   ${WHITE}On-chain governed spending limits for AI agents${NC}             ${MAGENTA}║${NC}"
    echo -e "${MAGENTA}║${NC}   ${YELLOW}Powered by Squads Protocol v4 · Superteam Brazil${NC}           ${MAGENTA}║${NC}"
    echo -e "${MAGENTA}║${NC}                                                               ${MAGENTA}║${NC}"
    echo -e "${MAGENTA}╚═══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
}

print_help() {
    echo "Cerberus Skill - Installer"
    echo ""
    echo "Usage: ./install.sh [OPTIONS]"
    echo ""
    echo "Installs cerberus-skill into ~/.claude/skills/cerberus/"
    echo ""
    echo "Options:"
    echo "  -y, --yes          Skip confirmation prompt"
    echo "  --no-claude-md     Do not update ~/.claude/CLAUDE.md"
    echo "  --target <PATH>    Custom install path (default: ~/.claude/skills/cerberus)"
    echo "  -h, --help         Show this help"
    echo ""
}

SKIP_CONFIRM=false
UPDATE_CLAUDE_MD=true
while [[ $# -gt 0 ]]; do
    case $1 in
        -y|--yes)        SKIP_CONFIRM=true;  shift ;;
        --no-claude-md)  UPDATE_CLAUDE_MD=false; shift ;;
        --target)        INSTALL_PATH="$2"; shift 2 ;;
        -h|--help)       print_help; exit 0 ;;
        *) echo "Unknown option: $1"; echo "Use --help for usage information"; exit 1 ;;
    esac
done

print_banner

echo -e "${WHITE}This will install:${NC}"
echo -e "  ${CYAN}•${NC} cerberus skill files  → ${CYAN}${INSTALL_PATH}/${NC}"
if [ "$UPDATE_CLAUDE_MD" = true ]; then
    echo -e "  ${CYAN}•${NC} CLAUDE.md             → ${CYAN}${CLAUDE_MD_PATH}${NC}"
fi
echo ""

if [ "$SKIP_CONFIRM" = false ]; then
    read -rp "Proceed? [Y/n] " -n 1
    echo
    if [[ $REPLY =~ ^[Nn]$ ]]; then
        echo -e "${YELLOW}Installation cancelled${NC}"
        exit 0
    fi
fi

echo ""

# Install skill files
echo -e "${CYAN}[1/2]${NC} Installing cerberus skill..."
if [ -d "$INSTALL_PATH" ]; then
    echo -e "  ${YELLOW}→${NC} Removing existing installation"
    rm -rf "$INSTALL_PATH"
fi
mkdir -p "$INSTALL_PATH"
cp -r "$SOURCE_DIR"/. "$INSTALL_PATH/"

# Install commands alongside the skill
if [ -d "$COMMANDS_DIR" ]; then
    mkdir -p "${HOME}/.claude/commands"
    cp "$COMMANDS_DIR"/*.md "${HOME}/.claude/commands/" 2>/dev/null || true
fi

echo -e "  ${GREEN}✓${NC} Installed to ${INSTALL_PATH}"

# Update CLAUDE.md
if [ "$UPDATE_CLAUDE_MD" = true ]; then
    echo -e "${CYAN}[2/2]${NC} Installing CLAUDE.md..."
    mkdir -p "${HOME}/.claude"
    if [ -f "$CLAUDE_MD_PATH" ]; then
        echo -e "  ${YELLOW}→${NC} Backing up existing CLAUDE.md to ${CLAUDE_MD_PATH}.backup"
        cp "$CLAUDE_MD_PATH" "${CLAUDE_MD_PATH}.backup"
    fi
    cp "$SCRIPT_DIR/CLAUDE.md" "$CLAUDE_MD_PATH"
    echo -e "  ${GREEN}✓${NC} Installed to ${CLAUDE_MD_PATH}"
fi

echo ""
echo -e "${GREEN}╔═══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║${NC}  ${WHITE}Cerberus Skill installed successfully!${NC}                       ${GREEN}║${NC}"
echo -e "${GREEN}╚═══════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${WHITE}Installed:${NC}"
echo -e "  ${GREEN}✓${NC} Skill files    ${CYAN}${INSTALL_PATH}/${NC}"
[ "$UPDATE_CLAUDE_MD" = true ] && echo -e "  ${GREEN}✓${NC} CLAUDE.md      ${CYAN}${CLAUDE_MD_PATH}${NC}"
[ -d "$COMMANDS_DIR" ] && echo -e "  ${GREEN}✓${NC} /cerberus-audit ${CYAN}~/.claude/commands/${NC}"
echo ""
echo -e "${CYAN}Try asking Claude:${NC}"
echo -e "  ${CYAN}•${NC} \"Create an on-chain spending limit for my AI agent wallet\""
echo -e "  ${CYAN}•${NC} \"My TX1–TX4 setup was interrupted — help me recover\""
echo -e "  ${CYAN}•${NC} \"/cerberus-audit <MULTISIG_PDA>\""
echo ""
echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${YELLOW}  github.com/joaopco8/cerberus-skill · Superteam Brazil${NC}"
echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
