#!/bin/sh
# The process runs as the unprivileged `yin` user. The container still starts as
# root so an existing deployment whose bind-mounted database/uploads directory
# was created by an older root-run image keeps working: repair the ownership
# once, then drop privileges before the server starts.
set -eu

if [ "$(id -u)" = "0" ]; then
    for directory in /app/database /app/uploads; do
        [ -d "$directory" ] || continue
        owner="$(stat -c %u "$directory")"
        if [ "$owner" != "10001" ]; then
            chown -R 10001:10001 "$directory"
        fi
    done
    exec setpriv --reuid=10001 --regid=10001 --init-groups "$@"
fi

exec "$@"
