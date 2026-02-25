#!/bin/bash
set -euo pipefail   # Exit on error, undefined vars, and pipe failures

# Usage: ./loop.sh [plan] [max_iterations] [--no-sandbox] [--goal <text>]
# Examples:
#   ./loop.sh plan 3                   # Plan mode, max 3 iters, docker
#   ./loop.sh plan 3 --no-sandbox      # Plan mode, max 3 iters, claude CLI + confirmation
#   ./loop.sh plan 3 --goal "feature"  # Plan mode, max 3 iters, custom goal
#   ./loop.sh plan                     # Plan mode, will ask you interactively for goal
#   ./loop.sh                          # Build mode, unlimited, via docker sandbox
#   ./loop.sh --no-sandbox             # Build mode, unlimited, via claude CLI directly + confirmation
#   ./loop.sh 20 --no-sandbox          # Build mode, max 20, claude CLI + confirmation

# Color and style definitions
GREEN_BOLD="\033[1;38;2;40;254;20m"    # #28FE14 + bold
YELLOW_BOLD="\033[1;33m"
RED_BOLD="\033[1;31m"
RESET="\033[0m"

# ────────────────────────────────────────────────
# Parse flags & positional arguments
# ────────────────────────────────────────────────

USE_SANDBOX=true
GOAL_TEXT=""
POSITIONAL=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-sandbox)
            USE_SANDBOX=false
            shift
            ;;
        --goal|--project-goal|-g|--project_specific_goal)
            shift
            [[ $# -eq 0 ]] && { echo -e "${RED_BOLD}Error: --goal requires a value${RESET}"; exit 1; }
            GOAL_TEXT="$1"
            shift
            ;;
        *)
            POSITIONAL+=("$1")
            shift
            ;;
    esac
done

set -- "${POSITIONAL[@]:-}"   # restore positional parameters

# ────────────────────────────────────────────────
# Mode, iterations, prompt file
# ────────────────────────────────────────────────

if [ "${1:-}" = "plan" ]; then
    MODE="plan"
    PROMPT_FILE="PROMPT_plan.md"
    shift
    MAX_ITERATIONS="${1:-0}"
    [[ "$MAX_ITERATIONS" =~ ^[0-9]+$ ]] && shift || MAX_ITERATIONS=0
elif [[ "${1:-}" =~ ^[0-9]+$ ]]; then
    MODE="build"
    PROMPT_FILE="PROMPT_build.md"
    MAX_ITERATIONS="$1"
    shift
else
    MODE="build"
    PROMPT_FILE="PROMPT_build.md"
    MAX_ITERATIONS=0
fi

# Last positional argument can be the goal (legacy style)
if [ -z "$GOAL_TEXT" ] && [ $# -ge 1 ]; then
    GOAL_TEXT="$1"
    shift
fi

# Project-specific goal (only interactive in plan mode if missing)
if [ "$MODE" = "plan" ] && [ -z "$GOAL_TEXT" ]; then
    # ──────────────────────────────
    # Very basic raw-mode editor hack for bash 3.2
    # ──────────────────────────────

    GOAL_TEXT=""           # starting suggestion
    cursor_pos=${#GOAL_TEXT} # where cursor is (end by default)

    # Save terminal state
    old_stty=$(stty -g 2>/dev/null)

    # Enter raw mode (char-by-char, no echo)
    stty -icanon -echo min 1 time 0 2>/dev/null

    # Hide real cursor
    printf '\033[?25l'

    trap '
        stty "$old_stty" 2>/dev/null
        printf "\033[?25h"          # show cursor again
        exit 1
    ' INT TERM EXIT

    while true; do
        # Clear + redraw whole editor view
        clear 2>/dev/null || printf '\033[H\033[2J'

        printf "${GREEN_BOLD}Type your goal - Enter to select project-specific goal for the agent to cycle planning${RESET}\n\n"

        # Move cursor back visually (crude)
        printf "\rAs our next objective, we want to achieve ${GREEN_BOLD}%s_${RESET}" "${GOAL_TEXT:0:$cursor_pos}"
        if [ $cursor_pos -lt ${#GOAL_TEXT} ]; then
            printf "${YELLOW_BOLD}%s_${RESET}" "${GOAL_TEXT:cursor_pos:1}"
        fi

        printf "\n\n${YELLOW_BOLD}Examples:${RESET}\n"
        printf "  • repo with full spec implementation\n"
        printf "  • a detailed analytics system based on specs/analytics.md\n"
        printf "  • a great working ui and ux\n"
        printf "  • an AI-powered data import system based on specs/data-import.md\n\n"
        # Read one character
        IFS= read -r -n 1 -d '' c 2>/dev/null

        case "$c" in
            # Enter → finish
            "" | $'\n')
                break
                ;;

            # Backspace / Ctrl+H
            $'\177' | $'\b')
                if [ $cursor_pos -gt 0 ]; then
                    GOAL_TEXT="${GOAL_TEXT:0:$((cursor_pos-1))}${GOAL_TEXT:$cursor_pos}"
                    cursor_pos=$((cursor_pos-1))
                fi
                ;;

            # Ctrl+C → abort (handled by trap)
            $'\003')
                # trap will run
                break
                ;;

            # Printable chars
            [[:print:]])
                # Insert at cursor position (very basic — only append + end cursor supported)
                GOAL_TEXT="${GOAL_TEXT:0:$cursor_pos}${c}${GOAL_TEXT:$cursor_pos}"
                cursor_pos=$((cursor_pos+1))
                ;;

            # Ignore control chars, arrows, etc.
            *)
                ;;
        esac
    done

    # Restore terminal + show cursor
    stty "$old_stty" 2>/dev/null
    printf '\033[?25h'
    trap - INT TERM EXIT

    # Final trim
    GOAL_TEXT=$(echo "$GOAL_TEXT" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')

    if [ -z "$GOAL_TEXT" ]; then
        echo -e "\nEmpty goal → exiting."
        exit 1
    fi

    # Confirmation screen
    clear 2>/dev/null || printf '\033[H\033[2J'
    echo -e "${GREEN_BOLD}Confirm goal${RESET}"
    echo -e "────────────────────────────────────────────────────────────────────"
    echo -e "As our next objective, we want to achieve ${GREEN_BOLD}${GOAL_TEXT}${RESET}."
    echo -e "────────────────────────────────────────────────────────────────────\n"
    echo -en "${GREEN_BOLD}Good? [Y/n] ${RESET}"
    read -n 1 -r confirm 2>/dev/null
    echo

    if [[ -n "$confirm" && ! "$confirm" =~ ^[Yy]$ ]]; then
        echo -e "Cancelled."
        exit 1
    fi
fi

# Prepare final prompt
if [ ! -f "$PROMPT_FILE" ]; then
    echo -e "${RED_BOLD}Error: $PROMPT_FILE not found${RESET}"
    exit 1
fi

PROMPT_CONTENT=$(cat "$PROMPT_FILE")

if [ -n "$GOAL_TEXT" ]; then
    # Simple literal replacement – safe and exact
    FINAL_PROMPT="${PROMPT_CONTENT//\[project-specific goal\]/$GOAL_TEXT}"
else
    FINAL_PROMPT="$PROMPT_CONTENT"
fi

# Select model
if [ "$MODE" = "build" ]; then
    MODEL="sonnet"   # for speed
else
    MODEL="opus"     # complex reasoning & planning
fi

# ────────────────────────────────────────────────
# Header
# ────────────────────────────────────────────────

ITERATION=0
CURRENT_BRANCH=$(git branch --show-current 2>/dev/null || echo "main")

echo -e "${GREEN_BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
echo -e "${GREEN_BOLD}Mode:         $MODE${RESET}"
echo -e "${GREEN_BOLD}Model:        $MODEL${RESET}"
echo -e "${GREEN_BOLD}Prompt:       $PROMPT_FILE${RESET}"
[ -n "$GOAL_TEXT" ] && echo -e "${GREEN_BOLD}Goal:         $GOAL_TEXT${RESET}"
echo -e "${GREEN_BOLD}Branch:       $CURRENT_BRANCH${RESET}"
echo -e "${GREEN_BOLD}Execution:    $(if $USE_SANDBOX; then echo "docker sandbox"; else echo "claude CLI (direct)${RESET}"; fi)${RESET}"
[ $MAX_ITERATIONS -gt 0 ] && echo -e "${GREEN_BOLD}Max:          $MAX_ITERATIONS iterations${RESET}"
echo -e "${GREEN_BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"

# ────────────────────────────────────────────────
# Confirmation when using --no-sandbox
# ────────────────────────────────────────────────

if ! $USE_SANDBOX; then
    echo -e ""
    echo -e "${YELLOW_BOLD}⚠️  WARNING: Running in DIRECT Claude CLI mode (--no-sandbox)${RESET}"
    echo -e "${YELLOW_BOLD}   • No sandbox isolation — Claude can run ANY shell command${RESET}"
    echo -e "${YELLOW_BOLD}   • --dangerously-skip-permissions is ON → all tool calls auto-approved${RESET}"
    echo -e "${YELLOW_BOLD}   • Model can read, write or delete files ANYWHERE your user has access${RESET}"
    echo -e "${YELLOW_BOLD}   • Only proceed if you accept full responsibility for the risk${RESET}"
    echo -e ""
    read -p "Continue without sandbox? (y/N) " -n 1 -r
    echo    # move to new line
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo -e "${RED_BOLD}Aborted by user.${RESET}"
        exit 1
    fi
    echo -e "${GREEN_BOLD}Confirmed — proceeding without sandbox.${RESET}\n"
fi

# ────────────────────────────────────────────────
# Main loop
# ────────────────────────────────────────────────

while true; do
    if [ $MAX_ITERATIONS -gt 0 ] && [ $ITERATION -ge $MAX_ITERATIONS ]; then
        echo -e "${GREEN_BOLD}Reached max iterations: $MAX_ITERATIONS${RESET}"
        break
    fi

    CURRENT_ITER=$((ITERATION + 1))

    START_TIME=$(date +%s)
    START_DISPLAY=$(date '+%Y-%m-%d %H:%M:%S')

    echo -e "${GREEN_BOLD}Starting iteration ${CURRENT_ITER} at ${START_DISPLAY}${RESET}"

    if $USE_SANDBOX; then
        # Run Ralph iteration via Docker sandbox (prompt passed directly)
        # -p: headless/non-interactive mode
        # --output-format=stream-json: structured streaming output
        # --model ...: selects the model
        # --verbose: detailed logging
        # Note: --dangerously-skip-permissions is automatic in sandbox
        docker sandbox run claude . -- \
            -p \
            --output-format=stream-json \
            --model "$MODEL" \
            --verbose \
            "$FINAL_PROMPT"
    else
        # Run Ralph iteration without sandbox with selected prompt
        # -p: Headless mode (non-interactive, reads from stdin)
        # --dangerously-skip-permissions: Auto-approve all tool calls (YOLO mode)
        # --output-format=stream-json: Structured output for logging/monitoring
        # --model ...: selects the model
        #               Can use 'sonnet' in build mode for speed if plan is clear and tasks well-defined
        # --verbose: Detailed execution logging
        echo "$FINAL_PROMPT" | claude -p \
            --output-format=stream-json \
            --model "$MODEL" \
            --verbose \
            --dangerously-skip-permissions
    fi

    END_TIME=$(date +%s)
    DURATION=$((END_TIME - START_TIME))
    DURATION_MIN=$((DURATION / 60))
    DURATION_SEC=$((DURATION % 60))

    echo -e "${GREEN_BOLD}Iteration ${CURRENT_ITER} completed in ${DURATION_MIN}m ${DURATION_SEC}s${RESET}"

    # Push changes after each iteration (non-fatal if it fails)
    git push origin "$CURRENT_BRANCH" 2>/dev/null || {
        echo -e "${GREEN_BOLD}Failed to push. Attempting to create remote branch...${RESET}"
        git push -u origin "$CURRENT_BRANCH" 2>/dev/null || echo -e "${YELLOW_BOLD}Git push failed, continuing...${RESET}"
    }

    # Completion check
    if [ -f .agent_complete ]; then
        echo -e "${GREEN_BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
        echo -e "${GREEN_BOLD}Agent signaled: COMPLETION${RESET}"
        echo -e "${GREEN_BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
        rm -f .agent_complete
        break
    fi

    ITERATION=$((ITERATION + 1))
    echo -e "${GREEN_BOLD}\n\n======================== LOOP $ITERATION ========================${RESET}\n"
done

echo -e "${GREEN_BOLD}Loop finished${RESET}"
