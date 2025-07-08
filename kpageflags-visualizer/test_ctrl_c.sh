#!/bin/bash

echo "Testing Ctrl-C interrupt functionality..."
echo "This will start scanning a large number of pages."
echo "Press Ctrl-C after a few seconds to test the interrupt handling."
echo ""

# Start the scan in background and kill it after 3 seconds to simulate Ctrl-C
timeout 3s sudo ./target/release/kpageflags-visualizer --count 1000000 --summary --histogram

echo ""
echo "Test completed - the scan should have been interrupted and shown a summary."
