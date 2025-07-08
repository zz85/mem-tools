#!/bin/bash

echo "KPageFlags Visualizer Examples"
echo "=============================="

echo -e "\n1. Basic analysis of first 20 pages:"
cargo run -- -c 20

echo -e "\n2. Verbose output for first 5 pages:"
cargo run -- -c 5 --verbose

echo -e "\n3. Summary only for 500 pages:"
cargo run -- -c 500 --summary

echo -e "\n4. Grid visualization:"
cargo run -- -c 200 --grid --width 40

echo -e "\n5. Analysis starting from a specific PFN:"
cargo run -- -s 0x1000 -c 50 --summary --grid
