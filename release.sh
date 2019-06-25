#!/bin/bash

set -e

if [ "$#" -ne 1 ]; then
	echo "usage: release.sh VERSION"
	exit 1
fi

RELEASE_FILE="$(mktemp)"
cat <<EOF > $RELEASE_FILE
dch-sm $1

## Quick Start

1.  Copy linux binary to \`\$PATH\`.
2.  Set \`DOCKER_SECRETSMANAGER_NAME\` to simple name or ARN of Secret.
3.  Set \`credStore\` to \`secretsmanager\` in \`\$HOME/.docker/config.json\`.
EOF

cargo test --release
git tag -s -m "dch-sm $1" $1
git push --tags
hub release create \
	-a target/release/docker-credential-secretsmanager \
	-F "$RELEASE_FILE" \
	$1
