#!/bin/bash
set -e
cd "$(dirname "$0")/web"
npm run build
echo "Web frontend built successfully."
