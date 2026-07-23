#!/bin/bash
# Move to the script's directory
cd "$(dirname "$0")"

echo "=================================================="
echo " Starting lynqo Local-First Server..."
echo "=================================================="

# Check if target binary exists, if not build it
if [ ! -f "./target/release/lynqo-server" ]; then
    echo "Release binary not found. Building first..."
    cargo build --release
fi

# Run server in the background
./target/release/lynqo-server &
SERVER_PID=$!

# Wait 1.5 seconds for the server to bind to the port
sleep 1.5

# Open default macOS browser to the dashboard
echo "Opening Dashboard: http://localhost:7432"
open "http://localhost:7432"

# Handle Ctrl+C gracefully to kill the server
cleanup() {
    echo ""
    echo "Stopping lynqo server..."
    kill $SERVER_PID
    exit 0
}
trap cleanup SIGINT SIGTERM

echo "lynqo is active! Keep this window open."
echo "Press Ctrl+C to close."
wait $SERVER_PID
