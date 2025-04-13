#!/bin/bash

# Step 1: Build the Docker image
docker build -t polar-bear-cross-compiler --platform "linux/amd64" .

# Step 2: Run the Docker container with the inputs as mounted volumes
docker run --platform=linux/amd64 --rm polar-bear-cross-compiler tar -cf - -C /output . > output.tar

# Step 3: Extract output
mkdir -p ./output && tar -xf output.tar -C ./output && rm output.tar