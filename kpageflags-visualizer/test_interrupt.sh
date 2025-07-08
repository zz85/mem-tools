#!/bin/bash

echo "Testing Ctrl-C interrupt functionality..."
echo "Starting scan of all pages - press Ctrl-C after a few seconds to test interrupt handling"
echo ""

sudo ./target/release/kpageflags-visualizer --summary --histogram
