#!/bin/sh
# Generate a cryptographically secure JWT secret
# Usage: ./gen_secret.sh [length]
#   length: number of random bytes (default 32, resulting in 43-char base64 string)

LENGTH="${1:-32}"
SECRET=$(head -c "$LENGTH" /dev/urandom | base64 | tr -d '=+/' | head -c "$LENGTH")
echo "$SECRET"
