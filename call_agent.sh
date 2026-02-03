#!/bin/bash

# Script to call Cursor Agent multiple times in a loop
# Usage: ./call_agent.sh [times] [agent_args...]
#   times: number of times to call agent (default: 5)
#   agent_args: arguments to pass to agent command

# Handle Ctrl+C (SIGINT) gracefully
CURRENT_CALL=0
cleanup() {
    echo ""
    echo "Interrupted by user (Ctrl+C)"
    if [ $CURRENT_CALL -gt 0 ]; then
        echo "Completed $CURRENT_CALL of $TIMES calls before interruption."
    fi
    exit 130
}
trap cleanup SIGINT

# Check if first argument is a number (times parameter)
if [[ "$1" =~ ^[1-9][0-9]*$ ]]; then
    # First argument is a number, use it as times
    TIMES=$1
    # Shift to remove the first argument, remaining args go to agent
    shift
else
    # First argument is not a number, use default times and pass all args to agent
    TIMES=5
fi

echo "Starting to call agent $TIMES times..."
echo ""

# Print arguments that will be passed to agent
if [ $# -eq 0 ]; then
    echo "Agent arguments: (none)"
else
    echo "Agent arguments: $@"
fi
echo ""

for i in $(seq 1 $TIMES); do
    CURRENT_CALL=$i
    echo "=== Call $i of $TIMES ==="
    agent "$@"
    echo ""
done

echo "Completed all $TIMES calls."
