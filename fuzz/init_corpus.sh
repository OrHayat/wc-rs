#!/bin/bash
# Initialize corpus from seeds
# Run this to reset corpus or when starting fresh

set -e

echo "Initializing corpus from seeds..."

# Clear corpus but keep directory
rm -rf corpus/shared/*

# Copy seeds to corpus as starting point
cp seeds/* corpus/shared/

echo "✓ Corpus initialized with $(ls seeds/ | wc -l) seed files"
echo "Ready to fuzz!"
